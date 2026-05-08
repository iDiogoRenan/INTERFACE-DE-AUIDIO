use serde::Serialize;
use std::path::PathBuf;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuracao invalida: {0}")]
    InvalidConfig(String),
    #[error("caminho invalido: {0}")]
    InvalidPath(PathBuf),
    #[error("codec nao suportado: {0}")]
    UnsupportedCodec(String),
    #[error("motor de fala nao configurado: {0}")]
    SpeechEngineUnavailable(String),
    #[error("traducao nao configurada: {0}")]
    TranslationUnavailable(String),
    #[error("job nao encontrado: {0}")]
    JobNotFound(String),
    #[error("erro de io: {0}")]
    Io(String),
    #[error("erro interno: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidConfig(value.to_string())
    }
}
