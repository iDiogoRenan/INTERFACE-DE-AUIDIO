use crate::{
    audio,
    error::{AppError, AppResult},
    project_metadata,
    speech::{
        runtime::SpeechRuntime, SynthesisCancellationCheck, SynthesisHooks,
        SynthesisProgressCallback, SynthesisRequest, Transcriber,
    },
    state::AppState,
    translation::{legacy_ptbr_postprocess, TranslationProvider},
};
use dublagem_domain::{
    DubbingJobEvent, DubbingRequest, JobEventKind, JobId, JobStage, TranslationRequest,
};
use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::{timeout as tokio_timeout, Duration};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const EVENT_STAGE: &str = "job:stage";
const EVENT_TRANSCRIPTION: &str = "job:transcription";
const EVENT_PROGRESS: &str = "job:progress";
const EVENT_FILE_COMPLETE: &str = "job:file-complete";
const EVENT_CANCELLED: &str = "job:cancelled";
const EVENT_FINISHED: &str = "job:finished";
const EVENT_FAILED: &str = "job:failed";
const MODEL_LOADING_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const TRANSCRIPTION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const TRANSLATION_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const SYNTHESIS_TIMEOUT: Duration = Duration::from_secs(20 * 60);

#[derive(Default)]
pub struct JobManager {
    active: Mutex<HashMap<JobId, CancellationToken>>,
}

impl JobManager {
    pub async fn register(&self, job_id: JobId) -> CancellationToken {
        let token = CancellationToken::new();
        self.active.lock().await.insert(job_id, token.clone());
        token
    }

    pub async fn cancel(&self, job_id: JobId) -> AppResult<()> {
        let token = self
            .active
            .lock()
            .await
            .remove(&job_id)
            .ok_or_else(|| AppError::JobNotFound(job_id.to_string()))?;
        token.cancel();
        Ok(())
    }

    pub async fn finish(&self, job_id: JobId) {
        self.active.lock().await.remove(&job_id);
    }
}

#[derive(Debug, Clone)]
struct FileContext {
    file_name: String,
    file_path: PathBuf,
    file_index: usize,
    total_files: usize,
}

impl FileContext {
    fn progress(&self, file_percent: u8) -> u8 {
        let completed_files = self.file_index as f32;
        let current_file = f32::from(file_percent.min(100)) / 100.0;
        (((completed_files + current_file) / self.total_files as f32) * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8
    }
}

pub async fn start_dubbing_job(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    request: DubbingRequest,
) -> AppResult<JobId> {
    let job_id = Uuid::new_v4();
    let cancellation = state.jobs.register(job_id).await;
    let jobs = Arc::clone(&state.jobs);
    let translator = Arc::clone(&state.translator);
    let speech = Arc::clone(&state.speech);

    tauri::async_runtime::spawn(async move {
        let result = run_job(
            app.clone(),
            job_id,
            cancellation,
            request,
            translator,
            speech,
        )
        .await;
        jobs.finish(job_id).await;

        if let Err(error) = result {
            let _ = emit(
                &app,
                EVENT_FAILED,
                event(
                    job_id,
                    JobEventKind::Failed,
                    Some(JobStage::Failed),
                    error.to_string(),
                    None,
                    None,
                ),
            );
        }
    });

    Ok(job_id)
}

async fn run_job(
    app: AppHandle,
    job_id: JobId,
    cancellation: CancellationToken,
    request: DubbingRequest,
    translator: Arc<dyn TranslationProvider>,
    speech: Arc<SpeechRuntime>,
) -> AppResult<()> {
    if request.input_paths.is_empty() {
        return Err(AppError::InvalidConfig(
            "selecione ao menos um audio de origem".to_string(),
        ));
    }
    project_metadata::validate_settings(&request.options.native_synthesis)?;
    for line in &request.line_overrides {
        project_metadata::validate_text_native_tags(&line.target_text)?;
        project_metadata::validate_native_tags(&line.tags)?;
        project_metadata::validate_settings(&line.settings)?;
    }

    std::fs::create_dir_all(&request.output_dir)?;
    let total = request.input_paths.len();
    emit_stage(
        &app,
        job_id,
        JobStage::LoadingModels,
        "Validando modelos locais e preparando motores ML.",
        Some(1),
        None,
    )?;

    let requested_model_dir = request.model_dir.clone();
    let speech_engines = cancellable_phase(
        &app,
        job_id,
        &cancellation,
        None,
        "validacao e carga de modelos",
        MODEL_LOADING_TIMEOUT,
        speech.engines(requested_model_dir),
    )
    .await?;
    let Some(speech_engines) = speech_engines else {
        emit_cancelled(&app, job_id, None)?;
        return Ok(());
    };
    let runtime_message = if speech_engines.reused_runtime {
        "Runtime ML residente reutilizado; modelos ja estavam carregados."
    } else {
        "Runtime ML carregado e mantido residente enquanto o app estiver aberto."
    };

    emit_stage(
        &app,
        job_id,
        JobStage::Queued,
        runtime_message,
        Some(2),
        None,
    )?;
    let transcriber = speech_engines.transcriber;
    let synthesizer = speech_engines.synthesizer;

    for (index, input_path) in request.input_paths.iter().enumerate() {
        let file_name = input_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("audio")
            .to_string();
        let context = FileContext {
            file_name,
            file_path: input_path.clone(),
            file_index: index,
            total_files: total,
        };

        if cancellation.is_cancelled() {
            emit_cancelled(&app, job_id, Some(&context))?;
            return Ok(());
        }

        emit_stage(
            &app,
            job_id,
            JobStage::PreparingFile,
            format!("Processando {}.", context.file_name),
            Some(context.progress(5)),
            Some(&context),
        )?;

        let source_text = resolve_source_text(
            &app,
            job_id,
            &context,
            &request,
            input_path,
            transcriber.as_ref(),
            &cancellation,
        )
        .await?;
        let Some(source_text) = source_text else {
            emit_cancelled(&app, job_id, Some(&context))?;
            return Ok(());
        };

        let target_text = resolve_target_text(
            &app,
            job_id,
            &context,
            &request,
            &source_text,
            translator.as_ref(),
            &cancellation,
        )
        .await?;
        let Some(target_text) = target_text else {
            emit_cancelled(&app, job_id, Some(&context))?;
            return Ok(());
        };

        let target_text = apply_text_options(target_text, source_text.clone(), &request.options)?;
        project_metadata::validate_text_native_tags(&target_text)?;
        emit_transcription(
            &app,
            job_id,
            &context,
            "Texto destino pronto para sintese.",
            context.progress(58),
            &source_text,
            Some(&target_text),
        )?;

        let output_path = request.output_dir.join(&context.file_name);
        let reference_audio = request.guide_audio.as_deref().unwrap_or(input_path);
        let synthesis_hooks =
            synthesis_hooks(app.clone(), job_id, context.clone(), cancellation.clone());
        emit_stage(
            &app,
            job_id,
            JobStage::Synthesizing,
            "Sintetizando voz com OmniVoice/Candle na GPU.",
            Some(context.progress(65)),
            Some(&context),
        )?;

        let synth_result = cancellable_phase(
            &app,
            job_id,
            &cancellation,
            Some(&context),
            "sintese OmniVoice",
            SYNTHESIS_TIMEOUT,
            synthesizer.synthesize(SynthesisRequest {
                text: &target_text,
                source_audio: input_path,
                reference_audio,
                output_path: &output_path,
                options: request.options.clone(),
                line_overrides: &request.line_overrides,
                hooks: synthesis_hooks,
            }),
        )
        .await;
        let synth_result = match synth_result {
            Ok(value) => value,
            Err(_) if cancellation.is_cancelled() => {
                emit_cancelled(&app, job_id, Some(&context))?;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        if synth_result.is_none() {
            emit_cancelled(&app, job_id, Some(&context))?;
            return Ok(());
        }

        emit_stage(
            &app,
            job_id,
            JobStage::WritingOutput,
            "Arquivo de saida escrito.",
            Some(context.progress(92)),
            Some(&context),
        )?;
        audio::save_transcription_cache(
            &request.output_dir,
            &context.file_name,
            &source_text,
            &target_text,
        )?;
        emit(
            &app,
            EVENT_FILE_COMPLETE,
            event(
                job_id,
                JobEventKind::FileComplete,
                Some(JobStage::FileComplete),
                "Arquivo concluido.",
                Some(context.progress(100)),
                Some(&context),
            )
            .with_text(Some(source_text), Some(target_text))
            .with_output_path(output_path),
        )?;
        emit_progress(&app, job_id, context.progress(100), Some(&context))?;
    }

    emit(
        &app,
        EVENT_FINISHED,
        event(
            job_id,
            JobEventKind::Finished,
            Some(JobStage::Finished),
            "Job concluido.",
            Some(100),
            None,
        )
        .with_total_files(total),
    )
}

async fn resolve_source_text(
    app: &AppHandle,
    job_id: JobId,
    context: &FileContext,
    request: &DubbingRequest,
    input_path: &Path,
    transcriber: &dyn Transcriber,
    cancellation: &CancellationToken,
) -> AppResult<Option<String>> {
    let source_text = request.custom_source_text.clone().unwrap_or_default();
    if !source_text.trim().is_empty() {
        emit_transcription(
            app,
            job_id,
            context,
            "Texto origem manual carregado.",
            context.progress(35),
            &source_text,
            None,
        )?;
        return Ok(Some(source_text));
    }

    emit_stage(
        app,
        job_id,
        JobStage::Transcribing,
        "Transcrevendo audio com Whisper local.",
        Some(context.progress(15)),
        Some(context),
    )?;
    let transcription = cancellable_phase(
        app,
        job_id,
        cancellation,
        Some(context),
        "transcricao Whisper",
        TRANSCRIPTION_TIMEOUT,
        transcriber.transcribe(
            input_path,
            request.options.source_language,
            request.options.target_language,
        ),
    )
    .await?;
    let Some(transcription) = transcription else {
        return Ok(None);
    };

    emit_transcription(
        app,
        job_id,
        context,
        "Transcricao concluida.",
        context.progress(35),
        &transcription.source_text,
        None,
    )?;
    Ok(Some(transcription.source_text))
}

async fn resolve_target_text(
    app: &AppHandle,
    job_id: JobId,
    context: &FileContext,
    request: &DubbingRequest,
    source_text: &str,
    translator: &dyn TranslationProvider,
    cancellation: &CancellationToken,
) -> AppResult<Option<String>> {
    let target_text = request.custom_target_text.clone().unwrap_or_default();
    if !target_text.trim().is_empty() {
        emit_transcription(
            app,
            job_id,
            context,
            "Texto destino manual carregado.",
            context.progress(55),
            source_text,
            Some(&target_text),
        )?;
        return Ok(Some(target_text));
    }

    emit_stage(
        app,
        job_id,
        JobStage::Translating,
        "Traduzindo texto para o idioma destino.",
        Some(context.progress(42)),
        Some(context),
    )?;
    let translation = cancellable_phase(
        app,
        job_id,
        cancellation,
        Some(context),
        "traducao",
        TRANSLATION_TIMEOUT,
        translator.translate(TranslationRequest {
            text: source_text.to_string(),
            source_language: request.options.source_language,
            target_language: request.options.target_language,
        }),
    )
    .await?;
    let Some(translation) = translation else {
        return Ok(None);
    };

    let target_text = legacy_ptbr_postprocess(
        &translation.translated_text,
        source_text,
        request.options.target_language,
    );
    emit_transcription(
        app,
        job_id,
        context,
        format!("Traducao concluida via {}.", translation.provider),
        context.progress(55),
        source_text,
        Some(&target_text),
    )?;
    Ok(Some(target_text))
}

async fn cancellable_phase<T, F>(
    app: &AppHandle,
    job_id: JobId,
    cancellation: &CancellationToken,
    context: Option<&FileContext>,
    label: &str,
    timeout_duration: Duration,
    future: F,
) -> AppResult<Option<T>>
where
    F: Future<Output = AppResult<T>>,
{
    tokio::select! {
        _ = cancellation.cancelled() => {
            emit_stage(
                app,
                job_id,
                JobStage::Cancelling,
                "Cancelamento solicitado; encerrando o job.",
                context.map(|item| item.progress(99)),
                context,
            )?;
            Ok(None)
        }
        result = tokio_timeout(timeout_duration, future) => {
            match result {
                Ok(value) => value.map(Some),
                Err(_) => {
                    cancellation.cancel();
                    Err(AppError::Internal(format!(
                        "{label} excedeu o limite de {} segundos",
                        timeout_duration.as_secs()
                    )))
                }
            }
        }
    }
}

fn synthesis_hooks(
    app: AppHandle,
    job_id: JobId,
    context: FileContext,
    cancellation: CancellationToken,
) -> SynthesisHooks {
    let progress_app = app.clone();
    let progress_context = context.clone();
    let progress: SynthesisProgressCallback = Arc::new(move |progress| {
        let total = progress.total_segments.max(1);
        let completed = progress.completed_segments.min(total);
        let synthesis_percent = 65
            + (((completed as f32 / total as f32) * 25.0)
                .round()
                .clamp(0.0, 25.0) as u8);
        let message = if completed == 0 {
            format!("Sintese GPU segmentada: {total} trechos preparados.")
        } else {
            format!("Sintese GPU: trecho {completed} de {total} concluido.")
        };
        let _ = emit_stage(
            &progress_app,
            job_id,
            JobStage::Synthesizing,
            message,
            Some(progress_context.progress(synthesis_percent)),
            Some(&progress_context),
        );
    });

    let should_cancel: SynthesisCancellationCheck = Arc::new(move || cancellation.is_cancelled());

    SynthesisHooks {
        progress: Some(progress),
        should_cancel: Some(should_cancel),
    }
}

trait JobEventExt {
    fn with_text(self, source_text: Option<String>, target_text: Option<String>) -> Self;
    fn with_output_path(self, output_path: impl Into<std::path::PathBuf>) -> Self;
    fn with_total_files(self, total_files: usize) -> Self;
}

impl JobEventExt for DubbingJobEvent {
    fn with_text(mut self, source_text: Option<String>, target_text: Option<String>) -> Self {
        self.source_text = source_text;
        self.target_text = target_text;
        self
    }

    fn with_output_path(mut self, output_path: impl Into<std::path::PathBuf>) -> Self {
        self.output_path = Some(output_path.into());
        self
    }

    fn with_total_files(mut self, total_files: usize) -> Self {
        self.total_files = Some(total_files);
        self
    }
}

fn apply_text_options(
    mut target_text: String,
    source_text: String,
    options: &dublagem_domain::DubbingOptions,
) -> AppResult<String> {
    if options.comma_before_question {
        target_text = crate::text::comma_before_question(&target_text);
    }
    if options.palatalize {
        target_text = crate::text::palatalize_ptbr(&target_text);
    }
    if options.trailing_period {
        target_text = format!("{} .", target_text.trim_end());
    }
    let target_text = crate::text::synchronize_punctuation(&target_text, &source_text);
    project_metadata::validate_text_native_tags(&target_text)?;
    Ok(target_text)
}

fn emit_stage(
    app: &AppHandle,
    job_id: JobId,
    stage: JobStage,
    message: impl Into<String>,
    progress: Option<u8>,
    context: Option<&FileContext>,
) -> AppResult<()> {
    emit(
        app,
        EVENT_STAGE,
        event(
            job_id,
            JobEventKind::Stage,
            Some(stage),
            message,
            progress,
            context,
        ),
    )?;
    if let Some(progress) = progress {
        emit_progress(app, job_id, progress, context)?;
    }
    Ok(())
}

fn emit_transcription(
    app: &AppHandle,
    job_id: JobId,
    context: &FileContext,
    message: impl Into<String>,
    progress: u8,
    source_text: &str,
    target_text: Option<&str>,
) -> AppResult<()> {
    let stage = if target_text.is_some() {
        JobStage::Translated
    } else {
        JobStage::Transcribed
    };
    emit(
        app,
        EVENT_TRANSCRIPTION,
        event(
            job_id,
            JobEventKind::Transcription,
            Some(stage),
            message,
            Some(progress),
            Some(context),
        )
        .with_text(
            Some(source_text.to_string()),
            target_text.map(str::to_string),
        ),
    )?;
    emit_progress(app, job_id, progress, Some(context))
}

fn emit_progress(
    app: &AppHandle,
    job_id: JobId,
    progress: u8,
    context: Option<&FileContext>,
) -> AppResult<()> {
    emit(
        app,
        EVENT_PROGRESS,
        event(
            job_id,
            JobEventKind::Progress,
            None,
            "Progresso atualizado.",
            Some(progress),
            context,
        ),
    )
}

fn emit_cancelled(app: &AppHandle, job_id: JobId, context: Option<&FileContext>) -> AppResult<()> {
    emit(
        app,
        EVENT_CANCELLED,
        event(
            job_id,
            JobEventKind::Cancelled,
            Some(JobStage::Cancelled),
            "Job cancelado pelo usuario.",
            context.map(|item| item.progress(100)),
            context,
        ),
    )
}

fn event(
    job_id: JobId,
    kind: JobEventKind,
    stage: Option<JobStage>,
    message: impl Into<String>,
    progress: Option<u8>,
    context: Option<&FileContext>,
) -> DubbingJobEvent {
    DubbingJobEvent {
        job_id,
        kind,
        stage,
        message: message.into(),
        progress,
        file_name: context.map(|item| item.file_name.clone()),
        file_path: context.map(|item| item.file_path.clone()),
        file_index: context.map(|item| item.file_index + 1),
        total_files: context.map(|item| item.total_files),
        source_text: None,
        target_text: None,
        output_path: None,
    }
}

fn emit(app: &AppHandle, event: &str, payload: DubbingJobEvent) -> AppResult<()> {
    app.emit(event, payload)
        .map_err(|error| AppError::Internal(error.to_string()))
}
