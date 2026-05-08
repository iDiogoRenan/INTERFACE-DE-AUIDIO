use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use dublagem_domain::{LanguageCode, TranslationRequest, TranslationResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait TranslationProvider: Send + Sync {
    async fn translate(&self, request: TranslationRequest) -> AppResult<TranslationResult>;
}

#[derive(Debug, Clone)]
pub struct GoogleCloudTranslateProvider {
    client: Client,
    project_id: Option<String>,
    access_token: Option<String>,
}

impl GoogleCloudTranslateProvider {
    pub fn from_environment() -> Self {
        Self {
            client: Client::new(),
            project_id: std::env::var("GOOGLE_CLOUD_PROJECT_ID").ok(),
            access_token: std::env::var("GOOGLE_CLOUD_ACCESS_TOKEN").ok(),
        }
    }

    fn credentials(&self) -> AppResult<(&str, &str)> {
        match (self.project_id.as_deref(), self.access_token.as_deref()) {
            (Some(project_id), Some(access_token)) if !project_id.is_empty() && !access_token.is_empty() => {
                Ok((project_id, access_token))
            }
            _ => Err(AppError::TranslationUnavailable(
                "defina GOOGLE_CLOUD_PROJECT_ID e GOOGLE_CLOUD_ACCESS_TOKEN para usar traducao automatica"
                    .to_string(),
            )),
        }
    }
}

#[async_trait]
impl TranslationProvider for GoogleCloudTranslateProvider {
    async fn translate(&self, request: TranslationRequest) -> AppResult<TranslationResult> {
        if request.source_language == request.target_language {
            return Ok(TranslationResult {
                translated_text: request.text,
                provider: "identity".to_string(),
            });
        }

        let target = request.target_language.translation_code().ok_or_else(|| {
            AppError::TranslationUnavailable("idioma destino invalido".to_string())
        })?;
        let (project_id, access_token) = self.credentials()?;
        let endpoint =
            format!("https://translation.googleapis.com/v3/projects/{project_id}:translateText");
        let source = request.source_language.translation_code();
        let body = GoogleTranslateRequest {
            contents: vec![request.text],
            mime_type: "text/plain",
            source_language_code: source,
            target_language_code: target,
        };

        let response = self
            .client
            .post(endpoint)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await
            .map_err(|error| AppError::TranslationUnavailable(error.to_string()))?;

        if !response.status().is_success() {
            return Err(AppError::TranslationUnavailable(format!(
                "Google Cloud Translation retornou HTTP {}",
                response.status()
            )));
        }

        let payload = response
            .json::<GoogleTranslateResponse>()
            .await
            .map_err(|error| AppError::TranslationUnavailable(error.to_string()))?;
        let translated_text = payload
            .translations
            .into_iter()
            .next()
            .map(|translation| translation.translated_text)
            .unwrap_or_default();

        Ok(TranslationResult {
            translated_text,
            provider: "google_cloud_translation_v3".to_string(),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTranslateRequest<'a> {
    contents: Vec<String>,
    mime_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_language_code: Option<&'a str>,
    target_language_code: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTranslateResponse {
    translations: Vec<GoogleTranslation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTranslation {
    translated_text: String,
}

pub fn legacy_ptbr_postprocess(
    text: &str,
    source_text: &str,
    target_language: LanguageCode,
) -> String {
    if target_language == LanguageCode::Pt {
        crate::text::synchronize_punctuation(
            &crate::text::correct_ptbr_pronunciation(text),
            source_text,
        )
    } else {
        crate::text::synchronize_punctuation(text, source_text)
    }
}
