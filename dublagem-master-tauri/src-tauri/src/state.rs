use crate::{
    jobs::JobManager,
    translation::{TranslationProvider, TranslationProviderChain},
};
use std::sync::Arc;

pub struct AppState {
    pub jobs: Arc<JobManager>,
    pub translator: Arc<dyn TranslationProvider>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(JobManager::default()),
            translator: Arc::new(TranslationProviderChain::from_environment()),
        }
    }
}
