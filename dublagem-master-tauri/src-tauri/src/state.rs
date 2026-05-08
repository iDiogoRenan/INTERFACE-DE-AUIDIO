use crate::{
    jobs::JobManager,
    speech::runtime::SpeechRuntime,
    translation::{TranslationProvider, TranslationProviderChain},
};
use std::sync::Arc;

pub struct AppState {
    pub jobs: Arc<JobManager>,
    pub speech: Arc<SpeechRuntime>,
    pub translator: Arc<dyn TranslationProvider>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(JobManager::default()),
            speech: Arc::new(SpeechRuntime::default()),
            translator: Arc::new(TranslationProviderChain::from_environment()),
        }
    }
}
