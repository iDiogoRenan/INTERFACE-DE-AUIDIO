use crate::{
    error::{AppError, AppResult},
    speech::{Transcriber, VoiceSynthesizer},
    state::AppState,
    translation::{legacy_ptbr_postprocess, TranslationProvider},
};
use dublagem_domain::{
    DubbingJobEvent, DubbingRequest, JobEventKind, JobId, TranscriptionResult, TranslationRequest,
};
use std::{collections::HashMap, path::Path, sync::Arc};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const EVENT_LOG: &str = "job:log";
const EVENT_PROGRESS: &str = "job:progress";
const EVENT_FILE_COMPLETE: &str = "job:file-complete";
const EVENT_FINISHED: &str = "job:finished";
const EVENT_FAILED: &str = "job:failed";

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

pub async fn start_dubbing_job(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    request: DubbingRequest,
) -> AppResult<JobId> {
    let job_id = Uuid::new_v4();
    let cancellation = state.jobs.register(job_id).await;
    let jobs = Arc::clone(&state.jobs);
    let transcriber = Arc::clone(&state.transcriber);
    let synthesizer = Arc::clone(&state.synthesizer);
    let translator = Arc::clone(&state.translator);

    tauri::async_runtime::spawn(async move {
        let result = run_job(
            app.clone(),
            job_id,
            cancellation,
            request,
            transcriber,
            synthesizer,
            translator,
        )
        .await;
        jobs.finish(job_id).await;

        if let Err(error) = result {
            let _ = emit(
                &app,
                EVENT_FAILED,
                DubbingJobEvent {
                    job_id,
                    kind: JobEventKind::Failed,
                    message: error.to_string(),
                    progress: None,
                    file_name: None,
                },
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
    transcriber: Arc<dyn Transcriber>,
    synthesizer: Arc<dyn VoiceSynthesizer>,
    translator: Arc<dyn TranslationProvider>,
) -> AppResult<()> {
    if request.input_paths.is_empty() {
        return Err(AppError::InvalidConfig(
            "selecione ao menos um audio de origem".to_string(),
        ));
    }

    std::fs::create_dir_all(&request.output_dir)?;
    let total = request.input_paths.len();
    emit_log(&app, job_id, "Job de dublagem iniciado.", None)?;

    for (index, input_path) in request.input_paths.iter().enumerate() {
        if cancellation.is_cancelled() {
            emit_log(&app, job_id, "Job cancelado pelo usuario.", None)?;
            return Ok(());
        }

        let file_name = input_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("audio")
            .to_string();
        emit_log(
            &app,
            job_id,
            &format!("Processando {file_name}."),
            Some(file_name.clone()),
        )?;

        let transcript = resolve_transcription(
            &request,
            input_path,
            transcriber.as_ref(),
            translator.as_ref(),
        )
        .await?;
        let source_text = transcript.source_text.clone();
        let target_text =
            apply_text_options(transcript.target_text, source_text.clone(), request.options);
        let output_path = request.output_dir.join(&file_name);
        let reference_audio = request.guide_audio.as_deref().unwrap_or(input_path);

        synthesizer
            .synthesize(
                &target_text,
                reference_audio,
                &source_text,
                &output_path,
                request.options,
            )
            .await?;

        let progress = (((index + 1) as f32 / total as f32) * 100.0).round() as u8;
        emit(
            &app,
            EVENT_FILE_COMPLETE,
            DubbingJobEvent {
                job_id,
                kind: JobEventKind::FileComplete,
                message: "Arquivo concluido.".to_string(),
                progress: Some(progress),
                file_name: Some(file_name),
            },
        )?;
        emit_progress(&app, job_id, progress)?;
    }

    emit(
        &app,
        EVENT_FINISHED,
        DubbingJobEvent {
            job_id,
            kind: JobEventKind::Finished,
            message: "Job concluido.".to_string(),
            progress: Some(100),
            file_name: None,
        },
    )
}

async fn resolve_transcription(
    request: &DubbingRequest,
    input_path: &Path,
    transcriber: &dyn Transcriber,
    translator: &dyn TranslationProvider,
) -> AppResult<TranscriptionResult> {
    let source_text = request.custom_source_text.clone().unwrap_or_default();
    let target_text = request.custom_target_text.clone().unwrap_or_default();

    if !source_text.trim().is_empty() && !target_text.trim().is_empty() {
        return Ok(TranscriptionResult {
            source_text,
            target_text,
            source_language: request.options.source_language,
            target_language: request.options.target_language,
        });
    }

    let transcription = transcriber
        .transcribe(
            input_path,
            request.options.source_language,
            request.options.target_language,
        )
        .await?;

    if !target_text.trim().is_empty() {
        return Ok(TranscriptionResult {
            target_text,
            ..transcription
        });
    }

    let translation = translator
        .translate(TranslationRequest {
            text: transcription.source_text.clone(),
            source_language: transcription.source_language,
            target_language: transcription.target_language,
        })
        .await?;
    Ok(TranscriptionResult {
        target_text: legacy_ptbr_postprocess(
            &translation.translated_text,
            &transcription.source_text,
            transcription.target_language,
        ),
        ..transcription
    })
}

fn apply_text_options(
    mut target_text: String,
    source_text: String,
    options: dublagem_domain::DubbingOptions,
) -> String {
    if options.comma_before_question {
        target_text = crate::text::comma_before_question(&target_text);
    }
    if options.palatalize {
        target_text = crate::text::palatalize_ptbr(&target_text);
    }
    if options.trailing_period {
        target_text = format!("{} .", target_text.trim_end());
    }
    crate::text::synchronize_punctuation(&target_text, &source_text)
}

fn emit_progress(app: &AppHandle, job_id: JobId, progress: u8) -> AppResult<()> {
    emit(
        app,
        EVENT_PROGRESS,
        DubbingJobEvent {
            job_id,
            kind: JobEventKind::Progress,
            message: "Progresso atualizado.".to_string(),
            progress: Some(progress),
            file_name: None,
        },
    )
}

fn emit_log(
    app: &AppHandle,
    job_id: JobId,
    message: &str,
    file_name: Option<String>,
) -> AppResult<()> {
    emit(
        app,
        EVENT_LOG,
        DubbingJobEvent {
            job_id,
            kind: JobEventKind::Log,
            message: message.to_string(),
            progress: None,
            file_name,
        },
    )
}

fn emit(app: &AppHandle, event: &str, payload: DubbingJobEvent) -> AppResult<()> {
    app.emit(event, payload)
        .map_err(|error| AppError::Internal(error.to_string()))
}
