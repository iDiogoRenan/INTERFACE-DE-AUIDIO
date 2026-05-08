use super::{
    models::{resolve_speech_model_paths, SpeechModelPaths},
    omnivoice::OmniVoiceCandleSynthesizer,
    whisper::WhisperRsTranscriber,
    Transcriber, VoiceSynthesizer,
};
use crate::error::AppResult;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct SpeechRuntime {
    model_paths: Mutex<Option<CachedModelPaths>>,
    transcriber: Mutex<Option<CachedTranscriber>>,
    synthesizer: Mutex<Option<CachedSynthesizer>>,
}

pub struct SpeechEngines {
    pub transcriber: Arc<dyn Transcriber>,
    pub synthesizer: Arc<dyn VoiceSynthesizer>,
    pub reused_runtime: bool,
}

struct CachedModelPaths {
    requested_model_dir: Option<PathBuf>,
    paths: SpeechModelPaths,
}

struct CachedTranscriber {
    model_path: PathBuf,
    engine: Arc<dyn Transcriber>,
}

struct CachedSynthesizer {
    model_dir: PathBuf,
    engine: Arc<dyn VoiceSynthesizer>,
}

struct ResolvedModelPaths {
    paths: SpeechModelPaths,
    was_cached: bool,
}

struct RuntimeHandle<T: ?Sized> {
    engine: Arc<T>,
    was_cached: bool,
}

impl SpeechRuntime {
    pub async fn engines(&self, model_dir: Option<PathBuf>) -> AppResult<SpeechEngines> {
        let model_paths = self.model_paths(model_dir).await?;
        let transcriber = self
            .transcriber_handle(model_paths.paths.whisper_model_path)
            .await?;
        let synthesizer = self
            .synthesizer_handle(model_paths.paths.omnivoice_model_dir)
            .await?;

        Ok(SpeechEngines {
            reused_runtime: model_paths.was_cached
                && transcriber.was_cached
                && synthesizer.was_cached,
            transcriber: transcriber.engine,
            synthesizer: synthesizer.engine,
        })
    }

    pub async fn transcriber_for_model_dir(
        &self,
        model_dir: Option<PathBuf>,
    ) -> AppResult<Arc<dyn Transcriber>> {
        let model_paths = self.model_paths(model_dir).await?;
        Ok(self
            .transcriber_handle(model_paths.paths.whisper_model_path)
            .await?
            .engine)
    }

    pub async fn synthesizer_for_model_dir(
        &self,
        model_dir: Option<PathBuf>,
    ) -> AppResult<Arc<dyn VoiceSynthesizer>> {
        let model_paths = self.model_paths(model_dir).await?;
        Ok(self
            .synthesizer_handle(model_paths.paths.omnivoice_model_dir)
            .await?
            .engine)
    }

    async fn model_paths(&self, model_dir: Option<PathBuf>) -> AppResult<ResolvedModelPaths> {
        let requested_model_dir = model_dir.map(normalize_existing_path).transpose()?;
        let mut model_cache = self.model_paths.lock().await;
        if let Some(cached) = model_cache
            .as_ref()
            .filter(|cached| cached.requested_model_dir == requested_model_dir)
        {
            return Ok(ResolvedModelPaths {
                paths: cached.paths.clone(),
                was_cached: true,
            });
        }

        let resolved_paths = tauri::async_runtime::spawn_blocking({
            let requested_model_dir = requested_model_dir.clone();
            move || {
                resolve_speech_model_paths(requested_model_dir.as_deref())
                    .and_then(normalize_model_paths)
            }
        })
        .await
        .map_err(|error| crate::error::AppError::Internal(error.to_string()))??;
        *model_cache = Some(CachedModelPaths {
            requested_model_dir,
            paths: resolved_paths.clone(),
        });

        Ok(ResolvedModelPaths {
            paths: resolved_paths,
            was_cached: false,
        })
    }

    async fn transcriber_handle(
        &self,
        model_path: PathBuf,
    ) -> AppResult<RuntimeHandle<dyn Transcriber>> {
        let model_path = normalize_existing_path(model_path)?;
        let mut cache = self.transcriber.lock().await;
        if let Some(cached) = cache
            .as_ref()
            .filter(|cached| cached.model_path == model_path)
        {
            return Ok(RuntimeHandle {
                engine: Arc::clone(&cached.engine),
                was_cached: true,
            });
        }

        let engine: Arc<dyn Transcriber> =
            Arc::new(WhisperRsTranscriber::preload(model_path.clone()).await?);
        *cache = Some(CachedTranscriber {
            model_path,
            engine: Arc::clone(&engine),
        });

        Ok(RuntimeHandle {
            engine,
            was_cached: false,
        })
    }

    async fn synthesizer_handle(
        &self,
        model_dir: PathBuf,
    ) -> AppResult<RuntimeHandle<dyn VoiceSynthesizer>> {
        let model_dir = normalize_existing_path(model_dir)?;
        let mut cache = self.synthesizer.lock().await;
        if let Some(cached) = cache
            .as_ref()
            .filter(|cached| cached.model_dir == model_dir)
        {
            return Ok(RuntimeHandle {
                engine: Arc::clone(&cached.engine),
                was_cached: true,
            });
        }

        let engine: Arc<dyn VoiceSynthesizer> =
            Arc::new(OmniVoiceCandleSynthesizer::preload(model_dir.clone()).await?);
        *cache = Some(CachedSynthesizer {
            model_dir,
            engine: Arc::clone(&engine),
        });

        Ok(RuntimeHandle {
            engine,
            was_cached: false,
        })
    }
}

fn normalize_model_paths(paths: SpeechModelPaths) -> AppResult<SpeechModelPaths> {
    Ok(SpeechModelPaths {
        whisper_model_path: normalize_existing_path(paths.whisper_model_path)?,
        omnivoice_model_dir: normalize_existing_path(paths.omnivoice_model_dir)?,
    })
}

fn normalize_existing_path(path: PathBuf) -> AppResult<PathBuf> {
    if path.exists() {
        Ok(path.canonicalize()?)
    } else {
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn reuses_validated_model_paths_for_equivalent_model_dir() {
        let temp_dir = tempdir().expect("temp dir");
        let model_dir = temp_dir.path().join("models");
        let whisper_dir = model_dir.join("whisper");
        let omnivoice_dir = model_dir.join("omnivoice");
        std::fs::create_dir_all(&whisper_dir).expect("whisper dir");
        std::fs::create_dir_all(&omnivoice_dir).expect("omnivoice dir");
        let whisper_model = whisper_dir.join("ggml-medium.bin");
        std::fs::write(&whisper_model, b"model").expect("whisper model");
        std::fs::write(omnivoice_dir.join("model.safetensors"), b"model").expect("omnivoice model");

        let runtime = SpeechRuntime::default();
        let first = runtime
            .model_paths(Some(model_dir.clone()))
            .await
            .expect("first resolution");
        std::fs::remove_file(whisper_model).expect("remove whisper model");
        let second = runtime
            .model_paths(Some(model_dir.join(".")))
            .await
            .expect("cached resolution");

        assert!(!first.was_cached);
        assert!(second.was_cached);
        assert_eq!(first.paths, second.paths);
    }
}
