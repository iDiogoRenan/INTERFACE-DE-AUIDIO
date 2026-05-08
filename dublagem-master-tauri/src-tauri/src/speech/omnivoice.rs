use super::{missing_model_error, ptbr_voice_profiles, VoiceSynthesizer};
use crate::error::AppResult;
use async_trait::async_trait;
use dublagem_domain::DubbingOptions;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct OmniVoiceCandleSynthesizer {
    model_dir: Option<PathBuf>,
}

impl OmniVoiceCandleSynthesizer {
    pub fn new(model_dir: Option<PathBuf>) -> Self {
        Self { model_dir }
    }
}

#[async_trait]
impl VoiceSynthesizer for OmniVoiceCandleSynthesizer {
    async fn synthesize(
        &self,
        _text: &str,
        _reference_audio: &Path,
        _reference_text: &str,
        _output_path: &Path,
        _options: DubbingOptions,
    ) -> AppResult<()> {
        let Some(model_dir) = &self.model_dir else {
            return Err(missing_model_error("OmniVoice Candle"));
        };

        synthesize_with_model(model_dir).await
    }

    async fn generate_voice_pool(&self, output_dir: &Path) -> AppResult<Vec<PathBuf>> {
        let Some(model_dir) = &self.model_dir else {
            return Err(missing_model_error("OmniVoice Candle"));
        };

        std::fs::create_dir_all(output_dir)?;
        generate_pool_with_model(model_dir, output_dir).await
    }
}

#[cfg(feature = "ml")]
async fn synthesize_with_model(_model_dir: &Path) -> AppResult<()> {
    Err(crate::error::AppError::SpeechEngineUnavailable(
        "adaptador OmniVoice/Candle exige o pin do repositório omnivoice-rs e manifesto dos pesos"
            .to_string(),
    ))
}

#[cfg(not(feature = "ml"))]
async fn synthesize_with_model(_model_dir: &Path) -> AppResult<()> {
    Err(crate::error::AppError::SpeechEngineUnavailable(
        "compile com --features ml para habilitar Candle/OmniVoice".to_string(),
    ))
}

async fn generate_pool_with_model(_model_dir: &Path, output_dir: &Path) -> AppResult<Vec<PathBuf>> {
    let profiles = ptbr_voice_profiles();
    let paths = profiles
        .into_iter()
        .map(|profile| output_dir.join(format!("{}.wav", profile.id)))
        .collect::<Vec<_>>();

    Err(crate::error::AppError::SpeechEngineUnavailable(format!(
        "pool PT-BR requer sintetese OmniVoice ativa; {} perfis declarados",
        paths.len()
    )))
}
