use super::Transcriber;
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use dublagem_domain::{LanguageCode, TranscriptionResult};
use std::path::{Path, PathBuf};
#[cfg(all(feature = "ml", feature = "cuda"))]
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct WhisperRsTranscriber {
    model_path: Option<PathBuf>,
    vad_model_path: Option<PathBuf>,
    #[cfg(all(feature = "ml", feature = "cuda"))]
    context: Option<Arc<whisper_rs::WhisperContext>>,
}

impl WhisperRsTranscriber {
    #[cfg(all(feature = "ml", feature = "cuda"))]
    pub async fn preload(model_path: PathBuf, vad_model_path: PathBuf) -> AppResult<Self> {
        let context = load_whisper_context(model_path.clone()).await?;
        Ok(Self {
            model_path: Some(model_path),
            vad_model_path: Some(vad_model_path),
            context: Some(context),
        })
    }

    #[cfg(all(feature = "ml", not(feature = "cuda")))]
    pub async fn preload(_model_path: PathBuf, _vad_model_path: PathBuf) -> AppResult<Self> {
        Err(AppError::SpeechEngineUnavailable(
            "Whisper requer GPU; compile com --features cuda para habilitar whisper-rs CUDA."
                .to_string(),
        ))
    }

    #[cfg(not(feature = "ml"))]
    pub async fn preload(model_path: PathBuf, vad_model_path: PathBuf) -> AppResult<Self> {
        Ok(Self {
            model_path: Some(model_path),
            vad_model_path: Some(vad_model_path),
        })
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

        #[cfg(all(feature = "ml", feature = "cuda"))]
        if let Some(context) = &self.context {
            return transcribe_with_context(
                Arc::clone(context),
                audio_path.to_path_buf(),
                vad_model_path.clone(),
                source_language,
                target_language,
            )
            .await;
        }

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

#[cfg(feature = "ml")]
async fn transcribe_with_model(
    model_path: &Path,
    vad_model_path: &Path,
    audio_path: &Path,
    source_language: LanguageCode,
    target_language: LanguageCode,
) -> AppResult<TranscriptionResult> {
    #[cfg(not(feature = "cuda"))]
    {
        let _ = (
            model_path,
            vad_model_path,
            audio_path,
            source_language,
            target_language,
        );
        Err(AppError::SpeechEngineUnavailable(
            "Whisper requer GPU; compile com --features cuda para habilitar whisper-rs CUDA."
                .to_string(),
        ))
    }

    #[cfg(feature = "cuda")]
    {
        let model_path = model_path.to_path_buf();
        let vad_model_path = vad_model_path.to_path_buf();
        let audio_path = audio_path.to_path_buf();
        let context = load_whisper_context(model_path).await?;
        transcribe_with_context(
            context,
            audio_path,
            vad_model_path,
            source_language,
            target_language,
        )
        .await
    }
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
async fn load_whisper_context(model_path: PathBuf) -> AppResult<Arc<whisper_rs::WhisperContext>> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut context_params = whisper_rs::WhisperContextParameters::default();
        context_params
            .use_gpu(true)
            .gpu_device(0)
            .dtw_parameters(whisper_rs::DtwParameters {
                mode: whisper_rs::DtwMode::ModelPreset {
                    model_preset: whisper_rs::DtwModelPreset::LargeV3,
                },
                ..whisper_rs::DtwParameters::default()
            });
        whisper_rs::WhisperContext::new_with_params(&model_path, context_params)
            .map(Arc::new)
            .map_err(|error| AppError::SpeechEngineUnavailable(error.to_string()))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[cfg(all(feature = "ml", feature = "cuda"))]
async fn transcribe_with_context(
    context: Arc<whisper_rs::WhisperContext>,
    audio_path: PathBuf,
    vad_model_path: PathBuf,
    source_language: LanguageCode,
    target_language: LanguageCode,
) -> AppResult<TranscriptionResult> {
    tauri::async_runtime::spawn_blocking(move || {
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
        params.set_token_timestamps(true);
        params.set_split_on_word(true);
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
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
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
