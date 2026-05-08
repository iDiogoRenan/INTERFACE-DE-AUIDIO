use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use dublagem_domain::{DubbingOptions, LanguageCode, TranscriptionResult, VoiceProfile};
use std::path::{Path, PathBuf};

pub mod omnivoice;
pub mod whisper;

#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(
        &self,
        audio_path: &Path,
        source_language: LanguageCode,
        target_language: LanguageCode,
    ) -> AppResult<TranscriptionResult>;
}

#[async_trait]
pub trait VoiceSynthesizer: Send + Sync {
    async fn synthesize(
        &self,
        text: &str,
        reference_audio: &Path,
        reference_text: &str,
        output_path: &Path,
        options: DubbingOptions,
    ) -> AppResult<()>;

    async fn generate_voice_pool(&self, output_dir: &Path) -> AppResult<Vec<PathBuf>>;
}

pub fn ptbr_voice_profiles() -> Vec<VoiceProfile> {
    vec![
        VoiceProfile {
            id: "male_adult".to_string(),
            instruct: "male, young adult, moderate pitch".to_string(),
            reference_text: "Ola, estou pronto para falar com voce hoje.".to_string(),
        },
        VoiceProfile {
            id: "female_adult".to_string(),
            instruct: "female, young adult, moderate pitch".to_string(),
            reference_text: "Ola, estou pronta para falar com voce.".to_string(),
        },
        VoiceProfile {
            id: "male_old".to_string(),
            instruct: "male, elderly, low pitch".to_string(),
            reference_text: "Ha muitos anos aprendi sobre essas coisas.".to_string(),
        },
        VoiceProfile {
            id: "female_old".to_string(),
            instruct: "female, elderly, moderate pitch".to_string(),
            reference_text: "Ha muito tempo aprendi que a paciencia e uma virtude.".to_string(),
        },
        VoiceProfile {
            id: "male_child".to_string(),
            instruct: "male, child, high pitch".to_string(),
            reference_text: "Ei, vamos brincar juntos hoje!".to_string(),
        },
        VoiceProfile {
            id: "female_child".to_string(),
            instruct: "female, child, high pitch".to_string(),
            reference_text: "Que dia lindo para uma aventura nova!".to_string(),
        },
    ]
}

pub fn missing_model_error(model: &str) -> AppError {
    AppError::SpeechEngineUnavailable(format!(
        "{model} ainda nao foi provisionado em models/. Baixe e registre o modelo antes de executar ML local."
    ))
}
