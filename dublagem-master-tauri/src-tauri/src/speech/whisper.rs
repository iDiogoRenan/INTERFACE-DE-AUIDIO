use super::Transcriber;
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use dublagem_domain::{LanguageCode, TranscriptionResult};
use std::path::{Path, PathBuf};

#[cfg(all(feature = "ml", feature = "cuda"))]
use chrono::{SecondsFormat, Utc};
#[cfg(all(feature = "ml", feature = "cuda"))]
use std::{process::Command, sync::Arc};
#[cfg(all(feature = "ml", feature = "cuda"))]
use uuid::Uuid;

#[cfg(all(feature = "ml", feature = "cuda"))]
const WORKER_ENV: &str = "NSG_DUB_WHISPER_WORKER";
#[cfg(all(feature = "ml", feature = "cuda"))]
const WORKER_MODEL_ENV: &str = "NSG_DUB_WHISPER_MODEL";
#[cfg(all(feature = "ml", feature = "cuda"))]
const WORKER_VAD_ENV: &str = "NSG_DUB_WHISPER_VAD";
#[cfg(all(feature = "ml", feature = "cuda"))]
const WORKER_AUDIO_ENV: &str = "NSG_DUB_WHISPER_AUDIO";
#[cfg(all(feature = "ml", feature = "cuda"))]
const WORKER_SOURCE_LANGUAGE_ENV: &str = "NSG_DUB_WHISPER_SOURCE_LANGUAGE";
#[cfg(all(feature = "ml", feature = "cuda"))]
const WORKER_TARGET_LANGUAGE_ENV: &str = "NSG_DUB_WHISPER_TARGET_LANGUAGE";
#[cfg(all(feature = "ml", feature = "cuda"))]
const WORKER_OUTPUT_ENV: &str = "NSG_DUB_WHISPER_OUTPUT";

#[cfg(all(feature = "ml", feature = "cuda"))]
struct WhisperWorkerFailureReport<'a> {
    executable: &'a Path,
    model_path: &'a Path,
    vad_model_path: &'a Path,
    audio_path: &'a Path,
    output: &'a std::process::Output,
    gpu_report: &'a crate::speech::gpu::CudaGpuReport,
}

#[derive(Debug, Clone, Default)]
pub struct WhisperRsTranscriber {
    model_path: Option<PathBuf>,
    vad_model_path: Option<PathBuf>,
}

impl WhisperRsTranscriber {
    #[cfg(any(all(feature = "ml", feature = "cuda"), not(feature = "ml")))]
    pub async fn preload(model_path: PathBuf, vad_model_path: PathBuf) -> AppResult<Self> {
        #[cfg(all(feature = "ml", feature = "cuda"))]
        crate::speech::gpu::require_cuda_gpu()?;

        Ok(Self {
            model_path: Some(model_path),
            vad_model_path: Some(vad_model_path),
        })
    }

    #[cfg(all(feature = "ml", not(feature = "cuda")))]
    pub async fn preload(_model_path: PathBuf, _vad_model_path: PathBuf) -> AppResult<Self> {
        Err(AppError::SpeechEngineUnavailable(
            "Whisper requer GPU; compile com --features cuda para habilitar whisper-rs CUDA."
                .to_string(),
        ))
    }
}

#[async_trait]
impl Transcriber for WhisperRsTranscriber {
    async fn transcribe(
        &self,
        audio_path: &Path,
        source_language: LanguageCode,
        target_language: LanguageCode,
    ) -> AppResult<TranscriptionResult> {
        let Some(model_path) = &self.model_path else {
            return Err(AppError::SpeechEngineUnavailable(
                "pasta de modelos não configurada. Selecione a pasta em Ajustes antes de dublar."
                    .to_string(),
            ));
        };
        let Some(vad_model_path) = &self.vad_model_path else {
            return Err(AppError::SpeechEngineUnavailable(
                "modelo VAD do Whisper não configurado. Selecione a pasta em Ajustes antes de dublar."
                    .to_string(),
            ));
        };

        transcribe_with_model(
            model_path,
            vad_model_path,
            audio_path,
            source_language,
            target_language,
        )
        .await
    }
}

pub(crate) fn run_worker_if_requested() -> Option<i32> {
    #[cfg(all(feature = "ml", feature = "cuda"))]
    {
        std::env::var_os(WORKER_ENV)?;
        Some(match run_worker_from_env() {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{error}");
                1
            }
        })
    }

    #[cfg(not(all(feature = "ml", feature = "cuda")))]
    {
        None
    }
}

#[cfg(all(feature = "ml", feature = "cuda"))]
async fn transcribe_with_model(
    model_path: &Path,
    vad_model_path: &Path,
    audio_path: &Path,
    source_language: LanguageCode,
    target_language: LanguageCode,
) -> AppResult<TranscriptionResult> {
    let model_path = model_path.to_path_buf();
    let vad_model_path = vad_model_path.to_path_buf();
    let audio_path = audio_path.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        run_isolated_worker(
            model_path,
            vad_model_path,
            audio_path,
            source_language,
            target_language,
        )
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[cfg(all(feature = "ml", not(feature = "cuda")))]
async fn transcribe_with_model(
    _model_path: &Path,
    _vad_model_path: &Path,
    _audio_path: &Path,
    _source_language: LanguageCode,
    _target_language: LanguageCode,
) -> AppResult<TranscriptionResult> {
    Err(AppError::SpeechEngineUnavailable(
        "Whisper requer GPU; compile com --features cuda para habilitar whisper-rs CUDA."
            .to_string(),
    ))
}

#[cfg(not(feature = "ml"))]
async fn transcribe_with_model(
    _model_path: &Path,
    _vad_model_path: &Path,
    _audio_path: &Path,
    _source_language: LanguageCode,
    _target_language: LanguageCode,
) -> AppResult<TranscriptionResult> {
    Err(AppError::SpeechEngineUnavailable(
        "compile com --features ml para habilitar whisper-rs".to_string(),
    ))
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn run_isolated_worker(
    model_path: PathBuf,
    vad_model_path: PathBuf,
    audio_path: PathBuf,
    source_language: LanguageCode,
    target_language: LanguageCode,
) -> AppResult<TranscriptionResult> {
    let executable = std::env::current_exe()?;
    let gpu_report = crate::speech::gpu::require_cuda_gpu()?;
    let output_path = worker_output_path();
    let output = Command::new(&executable)
        .env(WORKER_ENV, "1")
        .env(WORKER_MODEL_ENV, model_path.as_os_str())
        .env(WORKER_VAD_ENV, vad_model_path.as_os_str())
        .env(WORKER_AUDIO_ENV, audio_path.as_os_str())
        .env(WORKER_SOURCE_LANGUAGE_ENV, language_env(source_language))
        .env(WORKER_TARGET_LANGUAGE_ENV, language_env(target_language))
        .env(WORKER_OUTPUT_ENV, output_path.as_os_str())
        .env_remove("NSG_DUB_SUPERVISED")
        .output()
        .map_err(|error| AppError::SpeechEngineUnavailable(error.to_string()))?;

    if !output.status.success() {
        let report_path = write_worker_failure_report(WhisperWorkerFailureReport {
            executable: &executable,
            model_path: &model_path,
            vad_model_path: &vad_model_path,
            audio_path: &audio_path,
            output: &output,
            gpu_report: &gpu_report,
        })
        .ok();
        let _ = std::fs::remove_file(&output_path);
        return Err(AppError::SpeechEngineUnavailable(format!(
            "Whisper local falhou em processo isolado ({status}). A janela principal foi preservada.{report}",
            status = output.status,
            report = report_path
                .as_ref()
                .map(|path| format!(" Relatório: {}", path.display()))
                .unwrap_or_default()
        )));
    }

    let payload = std::fs::read(&output_path)?;
    let _ = std::fs::remove_file(&output_path);
    serde_json::from_slice::<TranscriptionResult>(&payload).map_err(AppError::from)
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn run_worker_from_env() -> AppResult<()> {
    let model_path = required_path_env(WORKER_MODEL_ENV)?;
    let vad_model_path = required_path_env(WORKER_VAD_ENV)?;
    let audio_path = required_path_env(WORKER_AUDIO_ENV)?;
    let output_path = required_path_env(WORKER_OUTPUT_ENV)?;
    let source_language = required_language_env(WORKER_SOURCE_LANGUAGE_ENV)?;
    let target_language = required_language_env(WORKER_TARGET_LANGUAGE_ENV)?;

    let transcription = transcribe_in_process(
        model_path,
        vad_model_path,
        audio_path,
        source_language,
        target_language,
    )?;
    let payload = serde_json::to_vec(&transcription)?;
    std::fs::write(output_path, payload)?;
    Ok(())
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn transcribe_in_process(
    model_path: PathBuf,
    vad_model_path: PathBuf,
    audio_path: PathBuf,
    source_language: LanguageCode,
    target_language: LanguageCode,
) -> AppResult<TranscriptionResult> {
    let context = load_whisper_context(model_path)?;
    let audio = crate::audio::read_audio_mono_16khz_f32(&audio_path)?;
    let vad_model_path = vad_model_path
        .to_str()
        .ok_or_else(|| AppError::InvalidPath(vad_model_path.clone()))?
        .to_string();
    let mut state = context
        .create_state()
        .map_err(|error| AppError::SpeechEngineUnavailable(error.to_string()))?;
    let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::BeamSearch {
        beam_size: 8,
        patience: 1.2,
    });
    params.set_language(whisper_language(source_language));
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_no_timestamps(false);
    params.set_token_timestamps(false);
    params.set_split_on_word(false);
    params.set_no_context(true);
    params.set_debug_mode(false);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_temperature(0.0);
    params.set_vad_model_path(Some(&vad_model_path));
    params.enable_vad(true);
    params.set_n_threads(whisper_thread_count());

    state
        .full(params, &audio)
        .map_err(|error| AppError::SpeechEngineUnavailable(error.to_string()))?;
    let source_text = state
        .as_iter()
        .map(|segment| segment.to_string())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if source_text.is_empty() {
        return Err(AppError::SpeechEngineUnavailable(
            "Whisper não encontrou segmentos de voz no áudio.".to_string(),
        ));
    }

    Ok(TranscriptionResult {
        source_text,
        target_text: String::new(),
        source_language,
        target_language,
    })
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn load_whisper_context(model_path: PathBuf) -> AppResult<Arc<whisper_rs::WhisperContext>> {
    let mut context_params = whisper_rs::WhisperContextParameters::default();
    context_params.use_gpu(true).gpu_device(0);
    whisper_rs::WhisperContext::new_with_params(&model_path, context_params)
        .map(Arc::new)
        .map_err(|error| AppError::SpeechEngineUnavailable(error.to_string()))
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn write_worker_failure_report(
    context: WhisperWorkerFailureReport<'_>,
) -> std::io::Result<PathBuf> {
    let report_dir = crate::crash_report::crash_report_dir();
    std::fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join(format!(
        "whisper-worker-{}-{}.log",
        Utc::now().format("%Y%m%d-%H%M%S%.3f"),
        std::process::id()
    ));
    let report = format!(
        "NSG Gaming Dub isolated Whisper worker failure\nTimestamp: {}\nExecutable: {}\n{}\nModel: {}\nVAD: {}\nAudio: {}\nStatus: {}\n\nstdout:\n{}\n\nstderr:\n{}\n",
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        context.executable.display(),
        context.gpu_report.diagnostic_line(),
        context.model_path.display(),
        context.vad_model_path.display(),
        context.audio_path.display(),
        context.output.status,
        String::from_utf8_lossy(&context.output.stdout),
        String::from_utf8_lossy(&context.output.stderr)
    );
    std::fs::write(&report_path, report)?;
    Ok(report_path)
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn required_path_env(key: &str) -> AppResult<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Internal(format!("variável de ambiente ausente: {key}")))
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn required_language_env(key: &str) -> AppResult<LanguageCode> {
    let value = std::env::var(key).map_err(|error| {
        AppError::Internal(format!("variável de idioma ausente {key}: {error}"))
    })?;
    language_from_env(&value)
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn worker_output_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "nsg-dub-whisper-{}-{}.json",
        std::process::id(),
        Uuid::new_v4()
    ))
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn language_env(language: LanguageCode) -> &'static str {
    language.as_bcp47().unwrap_or("auto")
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn language_from_env(value: &str) -> AppResult<LanguageCode> {
    match value {
        "auto" => Ok(LanguageCode::Auto),
        "en" => Ok(LanguageCode::En),
        "pt" => Ok(LanguageCode::Pt),
        "fr" => Ok(LanguageCode::Fr),
        "sv" => Ok(LanguageCode::Sv),
        unknown => Err(AppError::Internal(format!(
            "idioma inválido para worker Whisper: {unknown}"
        ))),
    }
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn whisper_thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|count| count.get().clamp(1, 8) as i32)
        .unwrap_or(4)
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn whisper_language(source_language: LanguageCode) -> Option<&'static str> {
    source_language.as_bcp47().or(Some("en"))
}
