use super::{missing_model_error, Transcriber};
use crate::error::AppResult;
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
        _audio_path: &Path,
        _source_language: LanguageCode,
        _target_language: LanguageCode,
    ) -> AppResult<TranscriptionResult> {
        let Some(model_path) = &self.model_path else {
            return Err(missing_model_error("whisper-rs ggml medium"));
        };

        transcribe_with_model(model_path).await
    }
}

#[cfg(feature = "ml")]
async fn transcribe_with_model(model_path: &Path) -> AppResult<TranscriptionResult> {
    let _ = whisper_rs::WhisperContext::new_with_params(
        model_path,
        whisper_rs::WhisperContextParameters::default(),
    )
    .map_err(|error| crate::error::AppError::SpeechEngineUnavailable(error.to_string()))?;

    Err(crate::error::AppError::SpeechEngineUnavailable(
        "pipeline whisper-rs precisa receber PCM 16 kHz mono antes da inferencia".to_string(),
    ))
}

#[cfg(not(feature = "ml"))]
async fn transcribe_with_model(_model_path: &Path) -> AppResult<TranscriptionResult> {
    Err(crate::error::AppError::SpeechEngineUnavailable(
        "compile com --features ml para habilitar whisper-rs".to_string(),
    ))
}
