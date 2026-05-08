use super::Transcriber;
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use dublagem_domain::{LanguageCode, TranscriptionResult};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct WhisperRsTranscriber {
    model_path: Option<PathBuf>,
}

impl WhisperRsTranscriber {
    pub fn new(model_path: Option<PathBuf>) -> Self {
        Self { model_path }
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
                "pasta de modelos nao configurada. Selecione a pasta em Ajustes antes de dublar."
                    .to_string(),
            ));
        };

        transcribe_with_model(model_path, audio_path, source_language, target_language).await
    }
}

#[cfg(feature = "ml")]
async fn transcribe_with_model(
    model_path: &Path,
    audio_path: &Path,
    source_language: LanguageCode,
    target_language: LanguageCode,
) -> AppResult<TranscriptionResult> {
    #[cfg(not(feature = "cuda"))]
    {
        let _ = (model_path, audio_path, source_language, target_language);
        Err(AppError::SpeechEngineUnavailable(
            "Whisper requer GPU; compile com --features cuda para habilitar whisper-rs CUDA."
                .to_string(),
        ))
    }

    #[cfg(feature = "cuda")]
    {
        let model_path = model_path.to_path_buf();
        let audio_path = audio_path.to_path_buf();
        tauri::async_runtime::spawn_blocking(move || {
            let audio = crate::audio::read_audio_mono_16khz_f32(&audio_path)?;
            let mut context_params = whisper_rs::WhisperContextParameters::default();
            context_params.use_gpu(true).gpu_device(0);
            let context = whisper_rs::WhisperContext::new_with_params(&model_path, context_params)
                .map_err(|error| AppError::SpeechEngineUnavailable(error.to_string()))?;
            let mut state = context
                .create_state()
                .map_err(|error| AppError::SpeechEngineUnavailable(error.to_string()))?;
            let mut params =
                whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(source_language.as_bcp47());
            params.set_translate(false);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_no_timestamps(true);
            params.set_no_context(true);
            params.set_debug_mode(false);
            params.set_suppress_blank(true);
            params.set_suppress_nst(true);
            params.set_temperature(0.0);
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
                    "Whisper nao encontrou segmentos de voz no audio.".to_string(),
                ));
            }

            Ok(TranscriptionResult {
                source_text,
                target_text: String::new(),
                source_language,
                target_language,
            })
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }
}

#[cfg(not(feature = "ml"))]
async fn transcribe_with_model(
    _model_path: &Path,
    _audio_path: &Path,
    _source_language: LanguageCode,
    _target_language: LanguageCode,
) -> AppResult<TranscriptionResult> {
    Err(AppError::SpeechEngineUnavailable(
        "compile com --features ml para habilitar whisper-rs".to_string(),
    ))
}

#[cfg(all(feature = "ml", feature = "cuda"))]
fn whisper_thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|count| count.get().clamp(1, 8) as i32)
        .unwrap_or(4)
}
