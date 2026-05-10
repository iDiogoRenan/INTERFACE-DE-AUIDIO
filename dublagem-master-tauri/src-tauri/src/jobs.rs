use crate::{
    audio,
    error::{AppError, AppResult},
    project_metadata,
    speech::{
        runtime::SpeechRuntime, SynthesisCancellationCheck, SynthesisHooks,
        SynthesisProgressCallback, SynthesisRequest, Transcriber, VoiceSynthesizer,
    },
    state::AppState,
    translation::{legacy_ptbr_postprocess, TranslationProvider},
};
use dublagem_domain::{
    DubbingJobEvent, DubbingOptions, DubbingRequest, JobEventKind, JobId, JobStage, LanguageCode,
    LineSynthesisOverride, TranslationRequest,
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
const V14_SYNTHESIS_ATTEMPTS: usize = 6;
const V14_VALIDATION_TIMEOUT: Duration = Duration::from_secs(10 * 60);

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
            "Sintetizando voz com rotina v14 OmniVoice/Candle na GPU.",
            Some(context.progress(65)),
            Some(&context),
        )?;

        let synthesis_result = synthesize_with_v14_guard(V14SynthesisJob {
            app: &app,
            job_id,
            context: &context,
            cancellation: &cancellation,
            synthesizer: synthesizer.as_ref(),
            transcriber: transcriber.as_ref(),
            target_text: &target_text,
            source_text: &source_text,
            source_audio: input_path,
            reference_audio,
            output_path: &output_path,
            options: &request.options,
            line_overrides: &request.line_overrides,
            hooks: synthesis_hooks,
        })
        .await?;
        if synthesis_result.is_none() {
            emit_cancelled(&app, job_id, Some(&context))?;
            return Ok(());
        }

        emit_stage(
            &app,
            job_id,
            JobStage::WritingOutput,
            "Arquivo de saida escrito pela rotina v14.",
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

struct V14SynthesisJob<'a> {
    app: &'a AppHandle,
    job_id: JobId,
    context: &'a FileContext,
    cancellation: &'a CancellationToken,
    synthesizer: &'a dyn VoiceSynthesizer,
    transcriber: &'a dyn Transcriber,
    target_text: &'a str,
    source_text: &'a str,
    source_audio: &'a Path,
    reference_audio: &'a Path,
    output_path: &'a Path,
    options: &'a DubbingOptions,
    line_overrides: &'a [LineSynthesisOverride],
    hooks: SynthesisHooks,
}

struct V14ValidationJob<'a> {
    app: &'a AppHandle,
    job_id: JobId,
    context: &'a FileContext,
    cancellation: &'a CancellationToken,
    transcriber: &'a dyn Transcriber,
    expected_text: &'a str,
    attempt_path: &'a Path,
    target_language: LanguageCode,
    attempt_number: usize,
}

#[derive(Debug, Clone)]
struct V14AttemptValidation {
    accepted: bool,
    message: String,
}

#[derive(Debug, Clone)]
struct V14TextMetrics {
    similarity: f32,
    coverage: f32,
    tail_ok: bool,
    expected_tokens: Vec<String>,
    heard_tokens: Vec<String>,
}

async fn synthesize_with_v14_guard(job: V14SynthesisJob<'_>) -> AppResult<Option<()>> {
    let mut attempt_paths = Vec::with_capacity(V14_SYNTHESIS_ATTEMPTS);
    let mut last_failure = String::new();
    let Some(reference_text) = resolve_v14_reference_text(&job).await? else {
        return Ok(None);
    };

    for attempt_index in 0..V14_SYNTHESIS_ATTEMPTS {
        if job.cancellation.is_cancelled() {
            cleanup_attempt_files(&attempt_paths);
            return Ok(None);
        }

        let attempt_number = attempt_index + 1;
        let attempt_path = synthesis_attempt_path(job.output_path, job.job_id, attempt_number);
        remove_file_if_exists(&attempt_path)?;
        attempt_paths.push(attempt_path.clone());

        let attempt_text = synthesis_text_with_final_breath(job.target_text, attempt_index);
        let attempt_line_overrides =
            line_overrides_with_final_breath(job.line_overrides, attempt_index);

        emit_stage(
            job.app,
            job.job_id,
            JobStage::Synthesizing,
            format!("Gerando tentativa v14 {attempt_number}/{V14_SYNTHESIS_ATTEMPTS}."),
            Some(job.context.progress(65)),
            Some(job.context),
        )?;

        let synth_result = cancellable_phase(
            job.app,
            job.job_id,
            job.cancellation,
            Some(job.context),
            "sintese OmniVoice v14",
            SYNTHESIS_TIMEOUT,
            job.synthesizer.synthesize(SynthesisRequest {
                text: &attempt_text,
                source_audio: job.source_audio,
                reference_audio: job.reference_audio,
                reference_text: &reference_text,
                output_path: &attempt_path,
                options: job.options.clone(),
                line_overrides: &attempt_line_overrides,
                hooks: job.hooks.clone(),
            }),
        )
        .await;

        match synth_result {
            Ok(Some(())) => {}
            Ok(None) => {
                cleanup_attempt_files(&attempt_paths);
                return Ok(None);
            }
            Err(_) if job.cancellation.is_cancelled() => {
                cleanup_attempt_files(&attempt_paths);
                return Ok(None);
            }
            Err(error) => {
                last_failure = error.to_string();
                emit_stage(
                    job.app,
                    job.job_id,
                    JobStage::Synthesizing,
                    format!("Tentativa v14 {attempt_number} falhou na sintese: {last_failure}"),
                    Some(job.context.progress(82)),
                    Some(job.context),
                )?;
                continue;
            }
        }

        let validation = validate_v14_attempt(V14ValidationJob {
            app: job.app,
            job_id: job.job_id,
            context: job.context,
            cancellation: job.cancellation,
            transcriber: job.transcriber,
            expected_text: job.target_text,
            attempt_path: &attempt_path,
            target_language: job.options.target_language,
            attempt_number,
        })
        .await?;

        if validation.accepted {
            copy_validated_attempt(&attempt_path, job.output_path)?;
            cleanup_attempt_files(&attempt_paths);
            emit_stage(
                job.app,
                job.job_id,
                JobStage::Synthesizing,
                format!("Tentativa v14 aprovada: {}.", validation.message),
                Some(job.context.progress(90)),
                Some(job.context),
            )?;
            return Ok(Some(()));
        }

        last_failure = validation.message;
        emit_stage(
            job.app,
            job.job_id,
            JobStage::Synthesizing,
            format!(
                "Tentativa v14 {attempt_number}/{V14_SYNTHESIS_ATTEMPTS} reprovada: {last_failure}"
            ),
            Some(job.context.progress(88)),
            Some(job.context),
        )?;
    }

    cleanup_attempt_files(&attempt_paths);
    Err(AppError::Internal(format!(
        "Sintese v14 reprovada apos {V14_SYNTHESIS_ATTEMPTS} tentativas: {last_failure}"
    )))
}

#[cfg(feature = "ml")]
async fn resolve_v14_reference_text(job: &V14SynthesisJob<'_>) -> AppResult<Option<String>> {
    if !v14_synthesis_uses_clone(job.options, job.line_overrides) {
        return Ok(Some(String::new()));
    }

    let reference_path = synthesis_reference_path(job.output_path, job.job_id);
    remove_file_if_exists(&reference_path)?;
    let reference = match audio::write_short_reference_wav(job.reference_audio, &reference_path) {
        Ok(reference) => reference,
        Err(error) => {
            let fallback = clean_reference_text(job.source_text);
            if fallback.is_empty() {
                return Err(error);
            }
            emit_stage(
                job.app,
                job.job_id,
                JobStage::Synthesizing,
                format!("Referencia curta v14 indisponivel; usando texto original: {error}"),
                Some(job.context.progress(63)),
                Some(job.context),
            )?;
            return Ok(Some(fallback));
        }
    };

    emit_stage(
        job.app,
        job.job_id,
        JobStage::Synthesizing,
        format!(
            "Transcrevendo referencia curta v14 ({:.2}s de {:.2}s, inicio {:.2}s).",
            reference.duration_seconds, reference.source_duration_seconds, reference.start_seconds
        ),
        Some(job.context.progress(63)),
        Some(job.context),
    )?;

    let reference_language = if same_audio_path(job.reference_audio, job.source_audio) {
        job.options.source_language
    } else {
        LanguageCode::Auto
    };
    let transcription = cancellable_phase(
        job.app,
        job.job_id,
        job.cancellation,
        Some(job.context),
        "transcricao da referencia curta v14",
        TRANSCRIPTION_TIMEOUT,
        job.transcriber.transcribe(
            &reference_path,
            reference_language,
            job.options.target_language,
        ),
    )
    .await;
    remove_file_if_exists(&reference_path)?;

    let Some(transcription) = (match transcription {
        Ok(value) => value,
        Err(error) => {
            let fallback = clean_reference_text(job.source_text);
            if fallback.is_empty() {
                return Err(error);
            }
            emit_stage(
                job.app,
                job.job_id,
                JobStage::Synthesizing,
                format!(
                    "Transcricao da referencia curta v14 falhou; usando texto original: {error}"
                ),
                Some(job.context.progress(64)),
                Some(job.context),
            )?;
            return Ok(Some(fallback));
        }
    }) else {
        return Ok(None);
    };

    let reference_text = clean_reference_text(&transcription.source_text);
    if !reference_text.is_empty() {
        return Ok(Some(reference_text));
    }

    let fallback = clean_reference_text(job.source_text);
    if fallback.is_empty() {
        return Err(AppError::SpeechEngineUnavailable(
            "Whisper nao obteve texto para a referencia curta v14".to_string(),
        ));
    }
    Ok(Some(fallback))
}

#[cfg(not(feature = "ml"))]
async fn resolve_v14_reference_text(job: &V14SynthesisJob<'_>) -> AppResult<Option<String>> {
    Ok(Some(job.source_text.to_string()))
}

#[cfg(any(feature = "ml", test))]
fn v14_synthesis_uses_clone(
    options: &DubbingOptions,
    line_overrides: &[LineSynthesisOverride],
) -> bool {
    if line_overrides.is_empty() {
        return matches!(
            options.native_synthesis.voice_mode,
            dublagem_domain::VoiceMode::Clone
        );
    }

    line_overrides
        .iter()
        .any(|line| matches!(line.settings.voice_mode, dublagem_domain::VoiceMode::Clone))
}

#[cfg(any(feature = "ml", test))]
fn clean_reference_text(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut previous_space = true;
    for character in text.chars() {
        if character.is_whitespace() {
            if !previous_space {
                cleaned.push(' ');
                previous_space = true;
            }
        } else if character.is_alphanumeric() {
            cleaned.push(character);
            previous_space = false;
        }
    }
    cleaned.trim().to_string()
}

async fn validate_v14_attempt(job: V14ValidationJob<'_>) -> AppResult<V14AttemptValidation> {
    if let Err(error) = validate_v14_audio_tail(job.attempt_path) {
        return Ok(V14AttemptValidation {
            accepted: false,
            message: error.to_string(),
        });
    }

    emit_stage(
        job.app,
        job.job_id,
        JobStage::Synthesizing,
        format!("Conferindo texto v14 na tentativa {}.", job.attempt_number),
        Some(job.context.progress(90)),
        Some(job.context),
    )?;

    let transcription = cancellable_phase(
        job.app,
        job.job_id,
        job.cancellation,
        Some(job.context),
        "conferencia Whisper v14 do audio gerado",
        V14_VALIDATION_TIMEOUT,
        job.transcriber
            .transcribe(job.attempt_path, job.target_language, job.target_language),
    )
    .await?;
    let Some(transcription) = transcription else {
        return Ok(V14AttemptValidation {
            accepted: false,
            message: "Cancelado durante conferencia Whisper.".to_string(),
        });
    };

    let heard_text = transcription.source_text.trim();
    let metrics = v14_text_metrics(job.expected_text, heard_text);
    let accepted = metrics.similarity >= 0.55
        && (metrics.expected_tokens.len() <= 2 || metrics.coverage >= 0.70)
        && metrics.tail_ok;
    let message = if accepted {
        format!(
            "Final completo (sim={:.2}, cobertura={:.2}).",
            metrics.similarity, metrics.coverage
        )
    } else if metrics.similarity < 0.55 {
        format!(
            "Texto divergente (sim={:.2}). Ouvido: '{}'",
            metrics.similarity, heard_text
        )
    } else if metrics.expected_tokens.len() > 2 && metrics.coverage < 0.70 {
        format!(
            "Palavras faltando (cobertura={:.2}). Ouvido: '{}'",
            metrics.coverage, heard_text
        )
    } else {
        format!(
            "Final incompleto. Esperado terminar com '{}', ouvido no fim: '{}'",
            metrics
                .expected_tokens
                .iter()
                .rev()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" "),
            metrics
                .heard_tokens
                .iter()
                .rev()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" ")
        )
    };

    Ok(V14AttemptValidation { accepted, message })
}

fn validate_v14_audio_tail(path: &Path) -> AppResult<()> {
    let samples = audio::read_wav_mono_f32(path)?;
    if samples.len() < 1_200 {
        return Err(AppError::Internal(
            "Audio final vazio/curto demais.".to_string(),
        ));
    }

    let metadata = audio::get_audio_metadata(path)?;
    let sample_rate = metadata.sample_rate.unwrap_or(24_000).max(1) as usize;
    let Some((active_start, active_end)) = active_sample_range_for_v14_validation(&samples) else {
        return Err(AppError::Internal(
            "Audio final sem fala audivel.".to_string(),
        ));
    };

    let voice = &samples[active_start..=active_end];
    let peak = voice
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()));
    if peak <= 1.0e-4 {
        return Err(AppError::Internal(
            "Audio gerado praticamente mudo.".to_string(),
        ));
    }

    let clipping_ratio = voice.iter().filter(|sample| sample.abs() > 0.985).count() as f32
        / voice.len().max(1) as f32;
    if clipping_ratio > 0.01 {
        return Err(AppError::Internal(format!(
            "Audio clipando/saturado ({:.1}%).",
            clipping_ratio * 100.0
        )));
    }

    let margin_samples = samples.len().saturating_sub(active_end + 1);
    let margin_ms = margin_samples as f32 * 1000.0 / sample_rate as f32;
    if margin_ms < 20.0 {
        return Err(AppError::Internal(format!(
            "Risco de corte no fim: so {margin_ms:.0} ms apos a ultima voz."
        )));
    }

    let voice_rms = rms_for_v14_validation(voice);
    let tail_samples = ((sample_rate as f32) * 0.04).round().max(1.0) as usize;
    let tail_start = samples.len().saturating_sub(tail_samples);
    let tail_rms = rms_for_v14_validation(&samples[tail_start..]);
    if tail_rms > (voice_rms * 0.08).max(1.0e-5) {
        return Err(AppError::Internal(
            "Ultimos 40 ms ainda tem energia de voz/ruido; risco de letra final cortada."
                .to_string(),
        ));
    }

    Ok(())
}

fn active_sample_range_for_v14_validation(samples: &[f32]) -> Option<(usize, usize)> {
    let peak = samples
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()));
    if peak <= 1.0e-4 {
        return None;
    }
    let threshold = (peak * 10_f32.powf(-38.0 / 20.0)).max(1.0e-4);
    let start = samples.iter().position(|sample| sample.abs() > threshold)?;
    let end = samples
        .iter()
        .rposition(|sample| sample.abs() > threshold)?;
    Some((start, end))
}

fn rms_for_v14_validation(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

fn v14_text_metrics(expected_text: &str, heard_text: &str) -> V14TextMetrics {
    let expected_similarity_text = normalized_similarity_text(expected_text);
    let heard_similarity_text = normalized_similarity_text(heard_text);
    let similarity = sequence_similarity(&expected_similarity_text, &heard_similarity_text);
    let expected_tokens = normalized_qc_tokens(expected_text);
    let heard_tokens = normalized_qc_tokens(heard_text);
    let coverage = if expected_tokens.is_empty() {
        1.0
    } else {
        fuzzy_lcs_count(&expected_tokens, &heard_tokens) as f32 / expected_tokens.len() as f32
    };
    let tail_ok = expected_tail_is_present(&expected_tokens, &heard_tokens);

    V14TextMetrics {
        similarity,
        coverage,
        tail_ok,
        expected_tokens,
        heard_tokens,
    }
}

fn normalized_similarity_text(value: &str) -> String {
    normalized_qc_tokens(value).join(" ")
}

fn normalized_qc_tokens(value: &str) -> Vec<String> {
    let mut normalized = String::new();
    for character in value.chars() {
        if let Some(folded) = fold_validation_char(character) {
            normalized.push(folded);
        } else {
            normalized.push(' ');
        }
    }

    for (from, to) in [
        ("dchi", "di"),
        ("dche", "de"),
        ("tchi", "ti"),
        ("tche", "te"),
        ("chi", "ti"),
        ("che", "te"),
    ] {
        normalized = normalized.replace(from, to);
    }

    normalized
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn fold_validation_char(character: char) -> Option<char> {
    let lower = character.to_lowercase().next().unwrap_or(character);
    if lower.is_ascii_alphanumeric() {
        return Some(lower);
    }

    match lower {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => Some('a'),
        'é' | 'è' | 'ê' | 'ë' => Some('e'),
        'í' | 'ì' | 'î' | 'ï' => Some('i'),
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => Some('o'),
        'ú' | 'ù' | 'û' | 'ü' => Some('u'),
        'ç' => Some('c'),
        'ñ' => Some('n'),
        _ => None,
    }
}

fn expected_tail_is_present(expected_tokens: &[String], heard_tokens: &[String]) -> bool {
    if expected_tokens.is_empty() {
        return true;
    }
    let meaningful_expected = expected_tokens
        .iter()
        .filter(|token| token.len() > 2)
        .collect::<Vec<_>>();
    let final_tokens = if meaningful_expected.is_empty() {
        expected_tokens.iter().collect::<Vec<_>>()
    } else {
        meaningful_expected
    };
    let final_tokens = final_tokens
        .into_iter()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let heard_window_len = 8_usize.max(final_tokens.len() + 3);
    let heard_window = heard_tokens
        .iter()
        .rev()
        .take(heard_window_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut position = 0;
    for token in final_tokens {
        let mut found = false;
        while position < heard_window.len() {
            if tokens_look_equal(token, heard_window[position]) {
                found = true;
                position += 1;
                break;
            }
            position += 1;
        }
        if !found {
            return false;
        }
    }
    true
}

fn fuzzy_lcs_count(expected: &[String], heard: &[String]) -> usize {
    if expected.is_empty() || heard.is_empty() {
        return 0;
    }

    let mut previous = vec![0; heard.len() + 1];
    for expected_token in expected {
        let mut current = vec![0; heard.len() + 1];
        for (heard_index, heard_token) in heard.iter().enumerate() {
            current[heard_index + 1] = if tokens_look_equal(expected_token, heard_token) {
                previous[heard_index] + 1
            } else {
                previous[heard_index + 1].max(current[heard_index])
            };
        }
        previous = current;
    }
    previous[heard.len()]
}

fn tokens_look_equal(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    if left.len() <= 2 || right.len() <= 2 {
        return false;
    }
    sequence_similarity(left, right) >= 0.78
}

fn sequence_similarity(left: &str, right: &str) -> f32 {
    let left_chars = left.chars().collect::<Vec<_>>();
    let right_chars = right.chars().collect::<Vec<_>>();
    if left_chars.is_empty() && right_chars.is_empty() {
        return 1.0;
    }
    if left_chars.is_empty() || right_chars.is_empty() {
        return 0.0;
    }

    let matches = lcs_char_count(&left_chars, &right_chars);
    (2.0 * matches as f32) / (left_chars.len() + right_chars.len()) as f32
}

fn lcs_char_count(left: &[char], right: &[char]) -> usize {
    let mut previous = vec![0; right.len() + 1];
    for left_char in left {
        let mut current = vec![0; right.len() + 1];
        for (right_index, right_char) in right.iter().enumerate() {
            current[right_index + 1] = if left_char == right_char {
                previous[right_index] + 1
            } else {
                previous[right_index + 1].max(current[right_index])
            };
        }
        previous = current;
    }
    previous[right.len()]
}

fn synthesis_text_with_final_breath(text: &str, attempt_index: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || attempt_index == 0 {
        return trimmed.to_string();
    }

    match attempt_index {
        1 => format!("{trimmed} ."),
        2 => format!("{trimmed} ..."),
        _ => format!("{trimmed} . ."),
    }
}

fn line_overrides_with_final_breath(
    line_overrides: &[LineSynthesisOverride],
    attempt_index: usize,
) -> Vec<LineSynthesisOverride> {
    let mut next = line_overrides.to_vec();
    if attempt_index == 0 {
        return next;
    }

    if let Some(line) = next
        .iter_mut()
        .rev()
        .find(|line| !line.target_text.trim().is_empty())
    {
        line.target_text = synthesis_text_with_final_breath(&line.target_text, attempt_index);
    }

    next
}

fn synthesis_attempt_path(output_path: &Path, job_id: JobId, attempt_number: usize) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dub");
    let file_name = format!("{stem}.{job_id}.attempt-{attempt_number}.wav");
    output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|parent| parent.join(&file_name))
        .unwrap_or_else(|| PathBuf::from(file_name))
}

#[cfg(feature = "ml")]
fn synthesis_reference_path(output_path: &Path, job_id: JobId) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dub");
    let file_name = format!("{stem}.{job_id}.reference.wav");
    output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|parent| parent.join(&file_name))
        .unwrap_or_else(|| PathBuf::from(file_name))
}

#[cfg(feature = "ml")]
fn same_audio_path(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn copy_validated_attempt(attempt_path: &Path, output_path: &Path) -> AppResult<()> {
    if let Some(parent) = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(attempt_path, output_path)?;
    Ok(())
}

fn cleanup_attempt_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = remove_file_if_exists(path);
    }
}

fn remove_file_if_exists(path: &Path) -> AppResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
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
        let message = if completed == 0 && total == 1 {
            "Sintese GPU v14: request unico preparado.".to_string()
        } else if completed == 0 {
            format!("Sintese GPU v14: {total} requests preparados.")
        } else if total == 1 {
            "Sintese GPU v14: request unico concluido.".to_string()
        } else {
            format!("Sintese GPU v14: request {completed} de {total} concluido.")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v14_metrics_accept_close_transcription() {
        let metrics =
            v14_text_metrics("Olá, posso ficar aqui agora?", "Ola posso ficar aqui agora");

        assert!(metrics.similarity >= 0.90);
        assert!(metrics.coverage >= 0.90);
        assert!(metrics.tail_ok);
    }

    #[test]
    fn v14_metrics_reject_missing_tail() {
        let metrics = v14_text_metrics(
            "Precisamos encontrar a chave antes do amanhecer",
            "Precisamos encontrar a chave",
        );

        assert!(!metrics.tail_ok);
        assert!(metrics.coverage < 0.75);
    }

    #[test]
    fn v14_retry_adds_final_breath_without_changing_first_attempt() {
        assert_eq!(
            synthesis_text_with_final_breath("fala final", 0),
            "fala final"
        );
        assert_eq!(
            synthesis_text_with_final_breath("fala final", 1),
            "fala final ."
        );
        assert_eq!(
            synthesis_text_with_final_breath("fala final", 2),
            "fala final ..."
        );
    }

    #[test]
    fn v14_retry_adds_final_breath_to_last_line_override_only() {
        let settings = dublagem_domain::NativeSynthesisSettings::default();
        let overrides = vec![
            LineSynthesisOverride {
                line_index: 0,
                target_text: "primeira linha".to_string(),
                tags: Vec::new(),
                settings: settings.clone(),
            },
            LineSynthesisOverride {
                line_index: 1,
                target_text: "ultima linha".to_string(),
                tags: Vec::new(),
                settings,
            },
        ];

        let next = line_overrides_with_final_breath(&overrides, 2);

        assert_eq!(next[0].target_text, "primeira linha");
        assert_eq!(next[1].target_text, "ultima linha ...");
    }

    #[test]
    fn cleans_reference_text_like_v14_prompt_text() {
        assert_eq!(
            clean_reference_text(" Olá, mundo!!!  Teste-42\nnovo."),
            "Olá mundo Teste42 novo"
        );
    }

    #[test]
    fn detects_clone_usage_from_global_or_line_settings() {
        let mut options = DubbingOptions::default();
        assert!(v14_synthesis_uses_clone(&options, &[]));

        options.native_synthesis.voice_mode = dublagem_domain::VoiceMode::Auto;
        let clone_line = LineSynthesisOverride {
            line_index: 0,
            target_text: "linha".to_string(),
            tags: Vec::new(),
            settings: dublagem_domain::NativeSynthesisSettings::default(),
        };

        assert!(!v14_synthesis_uses_clone(&options, &[]));
        assert!(v14_synthesis_uses_clone(&options, &[clone_line]));
    }
}
