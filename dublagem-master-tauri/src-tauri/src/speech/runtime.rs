use super::{
    omnivoice::OmniVoiceCandleSynthesizer, whisper::WhisperRsTranscriber, Transcriber,
    VoiceSynthesizer,
};
use crate::error::AppResult;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct SpeechRuntime {
    transcriber: Mutex<Option<CachedTranscriber>>,
    synthesizer: Mutex<Option<CachedSynthesizer>>,
}

struct CachedTranscriber {
    model_path: PathBuf,
    engine: Arc<dyn Transcriber>,
}

struct CachedSynthesizer {
    model_dir: PathBuf,
    engine: Arc<dyn VoiceSynthesizer>,
}

impl SpeechRuntime {
    pub async fn transcriber(&self, model_path: PathBuf) -> AppResult<Arc<dyn Transcriber>> {
        let mut cache = self.transcriber.lock().await;
        if let Some(cached) = cache
            .as_ref()
            .filter(|cached| cached.model_path == model_path)
        {
            return Ok(Arc::clone(&cached.engine));
        }

        let engine: Arc<dyn Transcriber> =
            Arc::new(WhisperRsTranscriber::preload(model_path.clone()).await?);
        *cache = Some(CachedTranscriber {
            model_path,
            engine: Arc::clone(&engine),
        });
        Ok(engine)
    }

    pub async fn synthesizer(&self, model_dir: PathBuf) -> AppResult<Arc<dyn VoiceSynthesizer>> {
        let mut cache = self.synthesizer.lock().await;
        if let Some(cached) = cache
            .as_ref()
            .filter(|cached| cached.model_dir == model_dir)
        {
            return Ok(Arc::clone(&cached.engine));
        }

        let engine: Arc<dyn VoiceSynthesizer> =
            Arc::new(OmniVoiceCandleSynthesizer::preload(model_dir.clone()).await?);
        *cache = Some(CachedSynthesizer {
            model_dir,
            engine: Arc::clone(&engine),
        });
        Ok(engine)
    }
}
