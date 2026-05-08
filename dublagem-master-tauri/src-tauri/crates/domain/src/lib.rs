use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub type JobId = Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageCode {
    #[default]
    Auto,
    En,
    Pt,
    Fr,
    Sv,
}

impl LanguageCode {
    pub const fn as_bcp47(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::En => Some("en"),
            Self::Pt => Some("pt"),
            Self::Fr => Some("fr"),
            Self::Sv => Some("sv"),
        }
    }

    pub const fn translation_code(self) -> Option<&'static str> {
        self.as_bcp47()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DubbingMode {
    #[default]
    Classico,
    Antisotaque,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFileStatus {
    #[default]
    Pending,
    Dubbed,
    Approved,
    Rejected,
    MissingSource,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingOptions {
    pub source_language: LanguageCode,
    pub target_language: LanguageCode,
    pub mode: DubbingMode,
    pub palatalize: bool,
    pub comma_before_question: bool,
    pub trailing_period: bool,
    pub pad_ms: u32,
    pub omni_temperature: f32,
}

impl Default for DubbingOptions {
    fn default() -> Self {
        Self {
            source_language: LanguageCode::Auto,
            target_language: LanguageCode::Pt,
            mode: DubbingMode::Classico,
            palatalize: false,
            comma_before_question: false,
            trailing_period: false,
            pad_ms: 200,
            omni_temperature: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub input_dir: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub guide_audio: Option<PathBuf>,
    pub approved_dir: Option<PathBuf>,
    pub model_dir: Option<PathBuf>,
    pub voice_pool_dir: Option<PathBuf>,
    pub options: DubbingOptions,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            input_dir: None,
            output_dir: None,
            guide_audio: None,
            approved_dir: None,
            model_dir: None,
            voice_pool_dir: Some(PathBuf::from("voice_pool_ptbr")),
            options: DubbingOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioMetadata {
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioFileEntry {
    pub name: String,
    pub path: PathBuf,
    pub family: String,
    pub status: AudioFileStatus,
    pub metadata: Option<AudioMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityReport {
    pub is_acceptable: bool,
    pub zcr_average: f32,
    pub peak_amplitude: f32,
    pub rms: f32,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub source_text: String,
    pub target_text: String,
    pub source_language: LanguageCode,
    pub target_language: LanguageCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingRequest {
    pub input_paths: Vec<PathBuf>,
    pub output_dir: PathBuf,
    pub guide_audio: Option<PathBuf>,
    pub model_dir: Option<PathBuf>,
    pub options: DubbingOptions,
    pub custom_source_text: Option<String>,
    pub custom_target_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobEventKind {
    Stage,
    Transcription,
    Progress,
    Log,
    FileComplete,
    Cancelled,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStage {
    Queued,
    LoadingModels,
    PreparingFile,
    Transcribing,
    Transcribed,
    Translating,
    Translated,
    Synthesizing,
    WritingOutput,
    FileComplete,
    Cancelling,
    Cancelled,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingJobEvent {
    pub job_id: JobId,
    pub kind: JobEventKind,
    pub stage: Option<JobStage>,
    pub message: String,
    pub progress: Option<u8>,
    pub file_name: Option<String>,
    pub file_index: Option<usize>,
    pub total_files: Option<usize>,
    pub source_text: Option<String>,
    pub target_text: Option<String>,
    pub output_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProfile {
    pub id: String,
    pub instruct: String,
    pub reference_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRequest {
    pub text: String,
    pub source_language: LanguageCode,
    pub target_language: LanguageCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResult {
    pub translated_text: String,
    pub provider: String,
}
