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

pub fn strip_omnivoice_native_tags(text: &str) -> String {
    let mut without_tags = String::with_capacity(text.len());
    let mut cursor = 0;

    while cursor < text.len() {
        let remaining = &text[cursor..];
        if let Some(tag) = omnivoice_native_tag_at(remaining) {
            without_tags.push(' ');
            cursor += tag.len();
            continue;
        }

        let Some(character) = remaining.chars().next() else {
            break;
        };
        without_tags.push(character);
        cursor += character.len_utf8();
    }

    normalize_speakable_tag_spacing(&without_tags)
}

fn omnivoice_native_tag_at(text: &str) -> Option<&'static str> {
    OMNIVOICE_NATIVE_TAGS
        .iter()
        .copied()
        .find(|tag| text.starts_with(tag))
}

fn normalize_speakable_tag_spacing(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut pending_space = false;

    for character in text.chars() {
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }

        if punctuation_without_leading_space(character) {
            pending_space = false;
            if normalized.ends_with(' ') {
                normalized.pop();
            }
        } else if pending_space {
            normalized.push(' ');
            pending_space = false;
        }

        normalized.push(character);
    }

    normalized.trim().to_string()
}

fn punctuation_without_leading_space(character: char) -> bool {
    matches!(
        character,
        ',' | '.'
            | '!'
            | '?'
            | ';'
            | ':'
            | '…'
            | ')'
            | ']'
            | '}'
            | '，'
            | '。'
            | '！'
            | '？'
            | '；'
            | '：'
            | '、'
            | '）'
            | '】'
    )
}

pub const OMNIVOICE_MAX_SYNTHESIS_SECONDS: f32 = 30.0;
pub const MIN_SYNTHESIS_CHUNKS: u32 = 1;
pub const DEFAULT_MAX_SYNTHESIS_CHUNKS: u32 = MIN_SYNTHESIS_CHUNKS;
pub const MAX_SYNTHESIS_CHUNKS: u32 = 20;

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechModelId {
    #[default]
    #[serde(rename = "omnivoice")]
    OmniVoice,
}

impl SpeechModelId {
    pub const ALL: [Self; 1] = [Self::OmniVoice];
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
        validate_optional_range(
            "durationSeconds",
            self.duration_seconds,
            0.25,
            OMNIVOICE_MAX_SYNTHESIS_SECONDS,
        )?;
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
            return Err("modo de voz por desenho requer instrução preenchida".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechModelPreset {
    pub native_synthesis: NativeSynthesisSettings,
}

pub fn default_speech_model_presets() -> BTreeMap<SpeechModelId, SpeechModelPreset> {
    SpeechModelId::ALL
        .into_iter()
        .map(|model_id| (model_id, SpeechModelPreset::default()))
        .collect()
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
    Ignored,
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
    #[serde(default = "default_max_synthesis_chunks")]
    pub max_synthesis_chunks: u32,
    #[serde(default)]
    pub preserve_sentence_boundaries: bool,
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
            max_synthesis_chunks: default_max_synthesis_chunks(),
            preserve_sentence_boundaries: false,
            native_synthesis: NativeSynthesisSettings::default(),
        }
    }
}

impl DubbingOptions {
    pub fn validate(&self) -> Result<(), String> {
        self.native_synthesis.validate()?;
        validate_integer_range(
            "maxSynthesisChunks",
            self.max_synthesis_chunks,
            MIN_SYNTHESIS_CHUNKS,
            MAX_SYNTHESIS_CHUNKS,
        )?;
        Ok(())
    }

    pub fn max_synthesis_duration_seconds(&self) -> f64 {
        max_synthesis_duration_seconds(self.max_synthesis_chunks)
    }
}

pub fn default_max_synthesis_chunks() -> u32 {
    DEFAULT_MAX_SYNTHESIS_CHUNKS
}

pub fn max_synthesis_duration_seconds(max_synthesis_chunks: u32) -> f64 {
    f64::from(OMNIVOICE_MAX_SYNTHESIS_SECONDS)
        * f64::from(max_synthesis_chunks.clamp(MIN_SYNTHESIS_CHUNKS, MAX_SYNTHESIS_CHUNKS))
}

fn validate_integer_range(
    name: &str,
    value: u32,
    minimum: u32,
    maximum: u32,
) -> Result<(), String> {
    if value < minimum || value > maximum {
        return Err(format!("{name} deve ficar entre {minimum} e {maximum}"));
    }
    Ok(())
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
    #[serde(default)]
    pub active_speech_model: SpeechModelId,
    #[serde(default)]
    pub speech_model_presets: BTreeMap<SpeechModelId, SpeechModelPreset>,
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
            active_speech_model: SpeechModelId::default(),
            speech_model_presets: default_speech_model_presets(),
            options: DubbingOptions::default(),
        }
    }
}

impl AppConfig {
    pub fn normalize_model_presets(mut self) -> Self {
        let active_model = self.active_speech_model;
        let legacy_active_settings = self.options.native_synthesis.clone();
        self.options.max_synthesis_chunks = self
            .options
            .max_synthesis_chunks
            .clamp(MIN_SYNTHESIS_CHUNKS, MAX_SYNTHESIS_CHUNKS);

        for model_id in SpeechModelId::ALL {
            self.speech_model_presets
                .entry(model_id)
                .or_insert_with(|| SpeechModelPreset {
                    native_synthesis: if model_id == active_model {
                        legacy_active_settings.clone()
                    } else {
                        NativeSynthesisSettings::default()
                    },
                });
        }

        if let Some(active_preset) = self.speech_model_presets.get(&active_model) {
            self.options.native_synthesis = active_preset.native_synthesis.clone();
        }

        self
    }

    pub fn with_active_native_synthesis(
        mut self,
        native_synthesis: NativeSynthesisSettings,
    ) -> Self {
        self.options.native_synthesis = native_synthesis.clone();
        self.speech_model_presets.insert(
            self.active_speech_model,
            SpeechModelPreset { native_synthesis },
        );
        self.normalize_model_presets()
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
    pub output_path: Option<PathBuf>,
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
    pub score: u8,
    pub classification: QualityClassification,
    pub summary: String,
    pub zcr_average: f32,
    pub peak_amplitude: f32,
    pub rms: f32,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityClassification {
    Excelente,
    Boa,
    Aceitavel,
    Ruim,
    Critica,
}

impl QualityClassification {
    pub const fn label_pt_br(self) -> &'static str {
        match self {
            Self::Excelente => "Excelente",
            Self::Boa => "Boa",
            Self::Aceitavel => "Aceitável",
            Self::Ruim => "Ruim",
            Self::Critica => "Crítica",
        }
    }
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
    #[serde(default)]
    pub save_output_as: Option<PathBuf>,
    pub guide_audio: Option<PathBuf>,
    pub model_dir: Option<PathBuf>,
    pub options: DubbingOptions,
    pub custom_source_text: Option<String>,
    pub custom_target_text: Option<String>,
    #[serde(default)]
    pub pinned_tags: Vec<String>,
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
    pub pinned_native_tags: Vec<String>,
    #[serde(default)]
    pub files: BTreeMap<String, ProjectFileMetadata>,
}

impl ProjectMetadata {
    pub fn v1() -> Self {
        Self {
            version: 1,
            pinned_native_tags: Vec::new(),
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
    pub timestamp: String,
    pub message: String,
    pub progress: Option<u8>,
    pub file_name: Option<String>,
    pub file_path: Option<PathBuf>,
    pub file_index: Option<usize>,
    pub total_files: Option<usize>,
    pub source_text: Option<String>,
    pub target_text: Option<String>,
    pub output_path: Option<PathBuf>,
    pub output_status: Option<AudioFileStatus>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_native_tags_from_speakable_text() {
        assert_eq!(
            strip_omnivoice_native_tags("[sigh] Ola [question-ah]?"),
            "Ola?"
        );
        assert_eq!(
            strip_omnivoice_native_tags("Inicio [laughter] meio [surprise-oh] fim."),
            "Inicio meio fim."
        );
    }

    #[test]
    fn keeps_pronunciation_hints_when_stripping_native_tags() {
        assert_eq!(
            strip_omnivoice_native_tags("[sigh] He plays [B EY1 S] guitar."),
            "He plays [B EY1 S] guitar."
        );
    }
}
