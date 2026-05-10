use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

pub type JobId = Uuid;

pub const OMNIVOICE_NATIVE_TAGS: &[&str] = &[
    "[laughter]",
    "[sigh]",
    "[confirmation-en]",
    "[question-en]",
    "[question-ah]",
    "[question-oh]",
    "[question-ei]",
    "[question-yi]",
    "[surprise-ah]",
    "[surprise-oh]",
    "[surprise-wa]",
    "[surprise-yo]",
    "[dissatisfaction-hnn]",
];

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
pub enum VoiceMode {
    #[default]
    Clone,
    Design,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeSynthesisSettings {
    pub voice_mode: VoiceMode,
    pub instruct: Option<String>,
    pub speed: Option<f32>,
    pub duration_seconds: Option<f32>,
    pub num_step: u32,
    pub guidance_scale: f32,
    pub position_temperature: f32,
    pub class_temperature: f32,
    pub denoise: bool,
    pub preprocess_prompt: bool,
    pub postprocess_output: bool,
    #[serde(default = "default_match_source_loudness")]
    pub match_source_loudness: bool,
    #[serde(default = "default_loudness_match_strength")]
    pub loudness_match_strength: f32,
    #[serde(default)]
    pub output_gain_db: f32,
    #[serde(default)]
    pub sibilance_reduction: f32,
    #[serde(default)]
    pub artifact_reduction: f32,
}

impl Default for NativeSynthesisSettings {
    fn default() -> Self {
        Self {
            voice_mode: VoiceMode::Clone,
            instruct: None,
            speed: None,
            duration_seconds: None,
            num_step: 48,
            guidance_scale: 2.0,
            position_temperature: 1.0,
            class_temperature: 0.0,
            denoise: true,
            preprocess_prompt: true,
            postprocess_output: true,
            match_source_loudness: default_match_source_loudness(),
            loudness_match_strength: default_loudness_match_strength(),
            output_gain_db: 0.0,
            sibilance_reduction: 0.0,
            artifact_reduction: 0.0,
        }
    }
}

impl NativeSynthesisSettings {
    pub fn validate(&self) -> Result<(), String> {
        validate_optional_range("speed", self.speed, 0.5, 2.0)?;
        validate_optional_range("durationSeconds", self.duration_seconds, 0.25, 60.0)?;
        validate_range("numStep", self.num_step as f32, 8.0, 128.0)?;
        validate_range("guidanceScale", self.guidance_scale, 0.0, 10.0)?;
        validate_range("positionTemperature", self.position_temperature, 0.0, 10.0)?;
        validate_range("classTemperature", self.class_temperature, 0.0, 10.0)?;
        validate_range(
            "loudnessMatchStrength",
            self.loudness_match_strength,
            0.0,
            1.0,
        )?;
        validate_range("outputGainDb", self.output_gain_db, -12.0, 12.0)?;
        validate_range("sibilanceReduction", self.sibilance_reduction, 0.0, 1.0)?;
        validate_range("artifactReduction", self.artifact_reduction, 0.0, 1.0)?;

        if matches!(self.voice_mode, VoiceMode::Design)
            && self
                .instruct
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
        {
            return Err("voiceMode design requer instruct preenchido".to_string());
        }

        Ok(())
    }
}

fn default_match_source_loudness() -> bool {
    false
}

fn default_loudness_match_strength() -> f32 {
    0.85
}

fn validate_optional_range(
    name: &str,
    value: Option<f32>,
    minimum: f32,
    maximum: f32,
) -> Result<(), String> {
    if let Some(value) = value {
        validate_range(name, value, minimum, maximum)?;
    }
    Ok(())
}

fn validate_range(name: &str, value: f32, minimum: f32, maximum: f32) -> Result<(), String> {
    if !value.is_finite() || value < minimum || value > maximum {
        return Err(format!(
            "{name} deve ficar entre {minimum:.2} e {maximum:.2}"
        ));
    }
    Ok(())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub native_synthesis: NativeSynthesisSettings,
}

impl Default for DubbingOptions {
    fn default() -> Self {
        Self {
            source_language: LanguageCode::En,
            target_language: LanguageCode::Pt,
            mode: DubbingMode::Classico,
            palatalize: false,
            comma_before_question: false,
            trailing_period: false,
            pad_ms: 200,
            omni_temperature: 0.0,
            native_synthesis: NativeSynthesisSettings::default(),
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
    pub transcription: Option<CachedTranscription>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedTranscription {
    pub source_text: String,
    pub target_text: String,
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
    #[serde(default)]
    pub line_overrides: Vec<LineSynthesisOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineSynthesisOverride {
    pub line_index: usize,
    pub target_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub settings: NativeSynthesisSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesisLinePreviewRequest {
    pub source_audio: PathBuf,
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub settings: NativeSynthesisSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub version: u16,
    #[serde(default)]
    pub files: BTreeMap<String, ProjectFileMetadata>,
}

impl ProjectMetadata {
    pub fn v1() -> Self {
        Self {
            version: 1,
            files: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileMetadata {
    #[serde(default)]
    pub source_text: Option<String>,
    #[serde(default)]
    pub target_text: Option<String>,
    #[serde(default)]
    pub baseline_source_text: Option<String>,
    #[serde(default)]
    pub baseline_target_text: Option<String>,
    #[serde(default)]
    pub lines: BTreeMap<String, ProjectLineMetadata>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLineMetadata {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub character_id: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub settings: NativeSynthesisSettings,
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
    pub file_path: Option<PathBuf>,
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
