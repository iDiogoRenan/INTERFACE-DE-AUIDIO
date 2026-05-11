use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use dublagem_domain::{
    DubbingOptions, LanguageCode, LineSynthesisOverride, TranscriptionResult, VoiceProfile,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub mod models;
pub mod omnivoice;
pub mod runtime;
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
    async fn synthesize(&self, request: SynthesisRequest<'_>) -> AppResult<()>;

    async fn generate_voice_pool(&self, output_dir: &Path) -> AppResult<Vec<PathBuf>>;
}

#[cfg_attr(not(feature = "ml"), allow(dead_code))]
pub struct SynthesisRequest<'a> {
    pub text: &'a str,
    pub source_audio: &'a Path,
    pub reference_audio: &'a Path,
    pub reference_text: &'a str,
    pub output_path: &'a Path,
    pub options: DubbingOptions,
    pub pinned_tags: &'a [String],
    pub line_overrides: &'a [LineSynthesisOverride],
    pub hooks: SynthesisHooks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "ml"), allow(dead_code))]
pub struct SynthesisProgress {
    pub completed_segments: usize,
    pub total_segments: usize,
}

impl SynthesisProgress {
    #[cfg(feature = "ml")]
    pub fn new(completed_segments: usize, total_segments: usize) -> Self {
        Self {
            completed_segments,
            total_segments: total_segments.max(1),
        }
    }
}

pub type SynthesisProgressCallback = Arc<dyn Fn(SynthesisProgress) + Send + Sync>;
pub type SynthesisCancellationCheck = Arc<dyn Fn() -> bool + Send + Sync>;

#[derive(Clone, Default)]
#[cfg_attr(not(feature = "ml"), allow(dead_code))]
pub struct SynthesisHooks {
    pub progress: Option<SynthesisProgressCallback>,
    pub should_cancel: Option<SynthesisCancellationCheck>,
}

impl SynthesisHooks {
    #[cfg(feature = "ml")]
    pub fn report(&self, completed_segments: usize, total_segments: usize) {
        if let Some(progress) = &self.progress {
            progress(SynthesisProgress::new(completed_segments, total_segments));
        }
    }

    #[cfg(feature = "ml")]
    pub fn is_cancelled(&self) -> bool {
        self.should_cancel
            .as_ref()
            .map(|should_cancel| should_cancel())
            .unwrap_or(false)
    }
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

pub fn missing_model_error(model: &str, expected_path: &Path) -> AppError {
    AppError::SpeechEngineUnavailable(format!(
        "{model} ainda não foi provisionado. Caminho esperado: {}. Baixe e registre o modelo antes de executar aprendizado de máquina local.",
        expected_path.display()
    ))
}
