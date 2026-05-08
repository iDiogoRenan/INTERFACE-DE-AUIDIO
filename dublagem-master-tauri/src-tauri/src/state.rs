use crate::{
    jobs::JobManager,
    speech::{omnivoice::OmniVoiceCandleSynthesizer, whisper::WhisperRsTranscriber},
    translation::GoogleCloudTranslateProvider,
};
use std::sync::Arc;

pub struct AppState {
    pub jobs: Arc<JobManager>,
    pub transcriber: Arc<WhisperRsTranscriber>,
    pub synthesizer: Arc<OmniVoiceCandleSynthesizer>,
    pub translator: Arc<GoogleCloudTranslateProvider>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(JobManager::default()),
            transcriber: Arc::new(WhisperRsTranscriber::new(None)),
            synthesizer: Arc::new(OmniVoiceCandleSynthesizer::new(None)),
            translator: Arc::new(GoogleCloudTranslateProvider::from_environment()),
        }
    }
}
