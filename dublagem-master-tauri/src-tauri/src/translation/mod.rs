use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use dublagem_domain::{LanguageCode, TranslationRequest, TranslationResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

const TRANSLATION_HTTP_TIMEOUT: Duration = Duration::from_secs(60);

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
            client: translation_client(),
            project_id: std::env::var("GOOGLE_CLOUD_PROJECT_ID").ok(),
            access_token: std::env::var("GOOGLE_CLOUD_ACCESS_TOKEN").ok(),
        }
    }

    fn is_configured(&self) -> bool {
        matches!(
            (self.project_id.as_deref(), self.access_token.as_deref()),
            (Some(project_id), Some(access_token))
                if !project_id.is_empty() && !access_token.is_empty()
        )
    }

    fn credentials(&self) -> AppResult<(&str, &str)> {
        match (self.project_id.as_deref(), self.access_token.as_deref()) {
            (Some(project_id), Some(access_token))
                if !project_id.is_empty() && !access_token.is_empty() =>
            {
                Ok((project_id, access_token))
            }
            _ => Err(AppError::TranslationUnavailable(
                "defina GOOGLE_CLOUD_PROJECT_ID e GOOGLE_CLOUD_ACCESS_TOKEN para usar tradução automática"
                    .to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GoogleTranslateCompatProvider {
    client: Client,
}

impl GoogleTranslateCompatProvider {
    pub fn new() -> Self {
        Self {
            client: translation_client(),
        }
    }
}

#[async_trait]
impl TranslationProvider for GoogleTranslateCompatProvider {
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
        let source = request.source_language.translation_code().unwrap_or("auto");
        let response = self
            .client
            .get("https://translate.googleapis.com/translate_a/single")
            .query(&[
                ("client", "gtx"),
                ("sl", source),
                ("tl", target),
                ("dt", "t"),
                ("q", request.text.as_str()),
            ])
            .send()
            .await
            .map_err(|error| AppError::TranslationUnavailable(error.to_string()))?;

        if !response.status().is_success() {
            return Err(AppError::TranslationUnavailable(format!(
                "Google Translate retornou HTTP {}",
                response.status()
            )));
        }

        let payload = response
            .json::<Value>()
            .await
            .map_err(|error| AppError::TranslationUnavailable(error.to_string()))?;
        let translated_text = extract_google_translate_text(&payload)?;

        Ok(TranslationResult {
            translated_text,
            provider: "google_translate_compat".to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct TranslationProviderChain {
    cloud: GoogleCloudTranslateProvider,
    compat: GoogleTranslateCompatProvider,
}

impl TranslationProviderChain {
    pub fn from_environment() -> Self {
        Self {
            cloud: GoogleCloudTranslateProvider::from_environment(),
            compat: GoogleTranslateCompatProvider::new(),
        }
    }
}

#[async_trait]
impl TranslationProvider for TranslationProviderChain {
    async fn translate(&self, request: TranslationRequest) -> AppResult<TranslationResult> {
        if self.cloud.is_configured() {
            return self.cloud.translate(request).await;
        }

        self.compat.translate(request).await
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

fn translation_client() -> Client {
    Client::builder()
        .timeout(TRANSLATION_HTTP_TIMEOUT)
        .build()
        .unwrap_or_else(|_| Client::new())
}

fn extract_google_translate_text(payload: &Value) -> AppResult<String> {
    let translated = payload
        .get(0)
        .and_then(Value::as_array)
        .map(|segments| {
            segments
                .iter()
                .filter_map(|segment| segment.get(0).and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default();

    if translated.trim().is_empty() {
        return Err(AppError::TranslationUnavailable(
            "resposta de tradução sem segmentos de texto".to_string(),
        ));
    }

    Ok(translated)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_google_translate_compat_segments() {
        let payload = json!([[
            ["Ola", "Hello", null, null],
            [" mundo", " world", null, null]
        ]]);

        assert_eq!(
            extract_google_translate_text(&payload).unwrap(),
            "Ola mundo"
        );
    }
}
