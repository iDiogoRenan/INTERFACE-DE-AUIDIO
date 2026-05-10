#[cfg(feature = "ml")]
use super::SynthesisHooks;
use super::{ptbr_voice_profiles, SynthesisRequest, VoiceSynthesizer};
#[cfg(feature = "ml")]
use crate::audio::{
    active_sample_range, audio_timing_profile, decode_audio_mono_f32, ms_to_samples,
    AudioTimingProfile,
};
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
#[cfg(feature = "ml")]
use dublagem_domain::{
    DubbingOptions, LanguageCode, LineSynthesisOverride, NativeSynthesisSettings, VoiceMode,
};
#[cfg(feature = "ml")]
use omnivoice_infer::{
    contracts::{
        DecodedAudio, GenerationRequest, ReferenceAudioInput, VoiceClonePrompt, WaveformInput,
    },
    pipeline::Phase3Pipeline,
    DTypeSpec, DeviceSpec, OmniVoiceError, RuntimeOptions,
};
use std::path::{Path, PathBuf};
#[cfg(feature = "ml")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "ml")]
const SYNTHESIS_SEGMENT_TARGET_SECONDS: f32 = 8.0;
#[cfg(feature = "ml")]
const SYNTHESIS_SEGMENT_MIN_CHARS: usize = 24;
#[cfg(feature = "ml")]
const SYNTHESIS_SEGMENT_MAX_CHARS: usize = 180;
#[cfg(feature = "ml")]
const INTERNAL_CHUNK_SECONDS: f32 = 15.0;
#[cfg(feature = "ml")]
const REFERENCE_TARGET_SECONDS: f32 = 8.0;
#[cfg(feature = "ml")]
const REFERENCE_MIN_SECONDS: f32 = 3.0;
#[cfg(feature = "ml")]
const REFERENCE_MAX_SECONDS: f32 = 10.0;

#[cfg(feature = "ml")]
struct ShortReferencePrompt {
    audio: ReferenceAudioInput,
    text: String,
}

#[cfg(feature = "ml")]
type SharedPhase3Pipeline = Arc<Mutex<Phase3Pipeline>>;

#[cfg(feature = "ml")]
struct OwnedSynthesisRequest {
    text: String,
    source_audio: PathBuf,
    reference_audio: PathBuf,
    reference_text: String,
    output_path: PathBuf,
    options: DubbingOptions,
    line_overrides: Vec<LineSynthesisOverride>,
    hooks: SynthesisHooks,
}

#[cfg(feature = "ml")]
impl OwnedSynthesisRequest {
    fn from_request(request: SynthesisRequest<'_>) -> Self {
        Self {
            text: request.text.to_string(),
            source_audio: request.source_audio.to_path_buf(),
            reference_audio: request.reference_audio.to_path_buf(),
            reference_text: request.reference_text.to_string(),
            output_path: request.output_path.to_path_buf(),
            options: request.options,
            line_overrides: request.line_overrides.to_vec(),
            hooks: request.hooks,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OmniVoiceCandleSynthesizer {
    model_dir: Option<PathBuf>,
    #[cfg(feature = "ml")]
    pipeline: Option<SharedPhase3Pipeline>,
}

impl OmniVoiceCandleSynthesizer {
    #[cfg(feature = "ml")]
    pub async fn preload(model_dir: PathBuf) -> AppResult<Self> {
        let pipeline = load_shared_pipeline(model_dir.clone()).await?;
        Ok(Self {
            model_dir: Some(model_dir),
            pipeline: Some(pipeline),
        })
    }

    #[cfg(not(feature = "ml"))]
    pub async fn preload(model_dir: PathBuf) -> AppResult<Self> {
        Ok(Self {
            model_dir: Some(model_dir),
        })
    }
}

#[async_trait]
impl VoiceSynthesizer for OmniVoiceCandleSynthesizer {
    async fn synthesize(&self, request: SynthesisRequest<'_>) -> AppResult<()> {
        let Some(model_dir) = &self.model_dir else {
            return Err(AppError::SpeechEngineUnavailable(
                "pasta de modelos nao configurada. Selecione a pasta em Ajustes antes de dublar."
                    .to_string(),
            ));
        };

        #[cfg(feature = "ml")]
        if let Some(pipeline) = &self.pipeline {
            return synthesize_with_pipeline(Arc::clone(pipeline), request).await;
        }

        synthesize_with_model(model_dir, request).await
    }

    async fn generate_voice_pool(&self, output_dir: &Path) -> AppResult<Vec<PathBuf>> {
        let Some(model_dir) = &self.model_dir else {
            return Err(AppError::SpeechEngineUnavailable(
                "pasta de modelos nao configurada. Selecione a pasta em Ajustes antes de dublar."
                    .to_string(),
            ));
        };

        std::fs::create_dir_all(output_dir)?;
        #[cfg(feature = "ml")]
        if let Some(pipeline) = &self.pipeline {
            return generate_pool_with_pipeline(Arc::clone(pipeline), output_dir).await;
        }

        generate_pool_with_model(model_dir, output_dir).await
    }
}

#[cfg(feature = "ml")]
async fn synthesize_with_model(model_dir: &Path, request: SynthesisRequest<'_>) -> AppResult<()> {
    let model_dir = model_dir.to_path_buf();
    let request = OwnedSynthesisRequest::from_request(request);

    tauri::async_runtime::spawn_blocking(move || {
        let pipeline = load_pipeline(model_dir)?;
        synthesize_blocking_with_pipeline(&pipeline, request)
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[cfg(feature = "ml")]
async fn synthesize_with_pipeline(
    pipeline: SharedPhase3Pipeline,
    request: SynthesisRequest<'_>,
) -> AppResult<()> {
    let request = OwnedSynthesisRequest::from_request(request);

    tauri::async_runtime::spawn_blocking(move || {
        let pipeline = pipeline
            .lock()
            .map_err(|error| AppError::Internal(error.to_string()))?;
        synthesize_blocking_with_pipeline(&pipeline, request)
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[cfg(not(feature = "ml"))]
async fn synthesize_with_model(_model_dir: &Path, _request: SynthesisRequest<'_>) -> AppResult<()> {
    Err(AppError::SpeechEngineUnavailable(
        "compile com --features ml para habilitar Candle/OmniVoice".to_string(),
    ))
}

#[cfg(feature = "ml")]
async fn generate_pool_with_model(model_dir: &Path, output_dir: &Path) -> AppResult<Vec<PathBuf>> {
    let model_dir = model_dir.to_path_buf();
    let output_dir = output_dir.to_path_buf();

    tauri::async_runtime::spawn_blocking(move || {
        let pipeline = load_pipeline(model_dir)?;
        generate_pool_blocking(&pipeline, output_dir)
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[cfg(feature = "ml")]
async fn generate_pool_with_pipeline(
    pipeline: SharedPhase3Pipeline,
    output_dir: &Path,
) -> AppResult<Vec<PathBuf>> {
    let output_dir = output_dir.to_path_buf();

    tauri::async_runtime::spawn_blocking(move || {
        let pipeline = pipeline
            .lock()
            .map_err(|error| AppError::Internal(error.to_string()))?;
        generate_pool_blocking(&pipeline, output_dir)
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[cfg(feature = "ml")]
fn generate_pool_blocking(
    pipeline: &Phase3Pipeline,
    output_dir: PathBuf,
) -> AppResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for profile in ptbr_voice_profiles() {
        let output_path = output_dir.join(format!("{}.wav", profile.id));
        if output_path.is_file() {
            paths.push(output_path);
            continue;
        }

        let mut request = GenerationRequest::new_text_only(profile.reference_text);
        request.languages = vec![Some("pt".to_string())];
        request.instructs = vec![Some(profile.instruct)];
        request.generation_config.num_step = 32;
        request.generation_config.guidance_scale = 2.0;
        request.generation_config.position_temperature = 1.0;
        request.generation_config.class_temperature = 0.0;
        request.generation_config.postprocess_output = true;

        let audio = pipeline
            .generate(&request)
            .map_err(map_omnivoice_error)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                AppError::SpeechEngineUnavailable(
                    "OmniVoice nao gerou audio para o perfil PT-BR.".to_string(),
                )
            })?;
        audio.write_wav(&output_path).map_err(map_omnivoice_error)?;
        paths.push(output_path);
    }

    Ok(paths)
}

#[cfg(not(feature = "ml"))]
async fn generate_pool_with_model(_model_dir: &Path, output_dir: &Path) -> AppResult<Vec<PathBuf>> {
    let profiles = ptbr_voice_profiles();
    let paths = profiles
        .into_iter()
        .map(|profile| output_dir.join(format!("{}.wav", profile.id)))
        .collect::<Vec<_>>();

    Err(crate::error::AppError::SpeechEngineUnavailable(format!(
        "pool PT-BR requer sintetese OmniVoice ativa; {} perfis declarados",
        paths.len()
    )))
}

#[cfg(feature = "ml")]
fn synthesize_blocking_with_pipeline(
    pipeline: &Phase3Pipeline,
    request: OwnedSynthesisRequest,
) -> AppResult<()> {
    let timing = audio_timing_profile(&request.source_audio, request.options.pad_ms)?;
    let target_duration = timing.target_voice_duration_seconds();
    let segments = synthesis_segments_for_request(&request, target_duration)?;
    if segments.is_empty() {
        return Err(AppError::InvalidConfig(
            "texto destino vazio; nao ha conteudo para sintese".to_string(),
        ));
    }
    request.hooks.report(0, segments.len());
    let voice_clone_prompt = if segments
        .iter()
        .any(|segment| matches!(segment.settings.voice_mode, VoiceMode::Clone))
    {
        let short_reference =
            prepare_short_reference(&request.reference_audio, &request.reference_text, timing)?;
        Some(
            pipeline
                .create_voice_clone_prompt_from_audio(
                    &short_reference.audio,
                    non_empty_str(&short_reference.text),
                    true,
                    None,
                )
                .map_err(map_omnivoice_error)?,
        )
    } else {
        None
    };
    let mut segment_audio = Vec::with_capacity(segments.len());

    for (index, segment) in segments.iter().enumerate() {
        if request.hooks.is_cancelled() {
            return Err(AppError::Internal("sintese cancelada".to_string()));
        }

        let audio = synthesize_segment(
            pipeline,
            &segment.text,
            segment.duration_seconds,
            voice_clone_prompt_for(&segment.settings, voice_clone_prompt.as_ref())?,
            &request.options,
            &segment.settings,
        )?;
        segment_audio.push(apply_segment_audio_polish(audio, &segment.settings));
        request.hooks.report(index + 1, segments.len());
    }

    let level_settings = audio_level_settings(&segments);
    let audio = concatenate_segments(segment_audio)?;
    let audio = apply_original_timing(audio, timing, level_settings);

    if let Some(parent) = request
        .output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    audio
        .write_wav(request.output_path)
        .map_err(map_omnivoice_error)
}

#[cfg(feature = "ml")]
#[derive(Debug, Clone)]
struct SynthesisSegmentPlan {
    text: String,
    duration_seconds: Option<f32>,
    settings: NativeSynthesisSettings,
}

#[cfg(feature = "ml")]
#[derive(Debug, Clone, Copy)]
struct AudioLevelSettings {
    match_source_loudness: bool,
    loudness_match_strength: f32,
    output_gain_db: f32,
}

#[cfg(feature = "ml")]
fn audio_level_settings(segments: &[SynthesisSegmentPlan]) -> AudioLevelSettings {
    let defaults = NativeSynthesisSettings::default();
    let Some(first) = segments.first() else {
        return AudioLevelSettings {
            match_source_loudness: defaults.match_source_loudness,
            loudness_match_strength: defaults.loudness_match_strength,
            output_gain_db: defaults.output_gain_db,
        };
    };

    let count = segments.len() as f32;
    AudioLevelSettings {
        match_source_loudness: segments
            .iter()
            .any(|segment| segment.settings.match_source_loudness),
        loudness_match_strength: segments
            .iter()
            .map(|segment| segment.settings.loudness_match_strength)
            .sum::<f32>()
            / count,
        output_gain_db: segments
            .iter()
            .map(|segment| segment.settings.output_gain_db)
            .sum::<f32>()
            / count,
    }
    .with_fallbacks(&first.settings)
}

#[cfg(feature = "ml")]
impl AudioLevelSettings {
    fn with_fallbacks(self, fallback: &NativeSynthesisSettings) -> Self {
        Self {
            match_source_loudness: self.match_source_loudness,
            loudness_match_strength: finite_or(
                self.loudness_match_strength,
                NativeSynthesisSettings::default().loudness_match_strength,
            )
            .clamp(0.0, 1.0),
            output_gain_db: finite_or(self.output_gain_db, fallback.output_gain_db)
                .clamp(-12.0, 12.0),
        }
    }
}

#[cfg(feature = "ml")]
fn synthesis_segments_for_request(
    request: &OwnedSynthesisRequest,
    target_duration: Option<f32>,
) -> AppResult<Vec<SynthesisSegmentPlan>> {
    if request.line_overrides.is_empty() {
        let text = request.text.trim();
        return Ok((!text.is_empty())
            .then(|| SynthesisSegmentPlan {
                text: text.to_string(),
                duration_seconds: effective_duration(
                    &request.options.native_synthesis,
                    target_duration,
                ),
                settings: request.options.native_synthesis.clone(),
            })
            .into_iter()
            .collect());
    }

    let mut segments = Vec::new();
    for line in synthesis_line_plans(&request.line_overrides, target_duration) {
        let text_segments = synthesis_segments(&line.text, line.duration_seconds);
        let durations = segment_durations(&text_segments, line.duration_seconds);
        segments.extend(text_segments.into_iter().zip(durations).map(
            |(text, duration_seconds)| SynthesisSegmentPlan {
                text,
                duration_seconds,
                settings: line.settings.clone(),
            },
        ));
    }

    Ok(segments)
}

#[cfg(feature = "ml")]
#[derive(Debug, Clone)]
struct SynthesisLinePlan {
    text: String,
    duration_seconds: Option<f32>,
    settings: NativeSynthesisSettings,
}

#[cfg(feature = "ml")]
fn synthesis_line_plans(
    overrides: &[LineSynthesisOverride],
    target_duration: Option<f32>,
) -> Vec<SynthesisLinePlan> {
    let mut sorted = overrides
        .iter()
        .filter(|line| !line.target_text.trim().is_empty())
        .collect::<Vec<_>>();
    sorted.sort_by_key(|line| line.line_index);
    let line_texts = sorted
        .iter()
        .map(|line| line.target_text.trim().to_string())
        .collect::<Vec<_>>();
    let inferred_durations = segment_durations(&line_texts, target_duration);

    sorted
        .into_iter()
        .zip(inferred_durations)
        .map(|(line, inferred_duration)| {
            let settings = line.settings.clone();
            SynthesisLinePlan {
                text: tagged_synthesis_text(&line.tags, line.target_text.trim()),
                duration_seconds: effective_duration(&settings, inferred_duration),
                settings,
            }
        })
        .collect()
}

#[cfg(feature = "ml")]
fn tagged_synthesis_text(tags: &[String], text: &str) -> String {
    let mut missing_tags = Vec::new();
    for tag in tags {
        let tag = tag.as_str();
        if dublagem_domain::OMNIVOICE_NATIVE_TAGS.contains(&tag)
            && !text.contains(tag)
            && !missing_tags.contains(&tag)
        {
            missing_tags.push(tag);
        }
    }
    if missing_tags.is_empty() {
        return text.to_string();
    }

    format!("{} {}", missing_tags.join(" "), text)
        .trim()
        .to_string()
}

#[cfg(feature = "ml")]
fn effective_duration(
    settings: &NativeSynthesisSettings,
    inferred_duration: Option<f32>,
) -> Option<f32> {
    settings.duration_seconds.or_else(|| {
        if settings.speed.is_some() {
            None
        } else {
            inferred_duration
        }
    })
}

#[cfg(feature = "ml")]
fn voice_clone_prompt_for<'a>(
    settings: &NativeSynthesisSettings,
    prompt: Option<&'a VoiceClonePrompt>,
) -> AppResult<Option<&'a VoiceClonePrompt>> {
    if !matches!(settings.voice_mode, VoiceMode::Clone) {
        return Ok(None);
    }

    prompt.map(Some).ok_or_else(|| {
        AppError::SpeechEngineUnavailable("prompt de voz clone indisponivel".to_string())
    })
}

#[cfg(feature = "ml")]
fn synthesize_segment(
    pipeline: &Phase3Pipeline,
    text: &str,
    target_duration_seconds: Option<f32>,
    voice_clone_prompt: Option<&VoiceClonePrompt>,
    options: &DubbingOptions,
    settings: &NativeSynthesisSettings,
) -> AppResult<DecodedAudio> {
    let mut request = generation_request(
        text.to_string(),
        voice_clone_prompt,
        target_duration_seconds,
        options,
        settings,
    );
    request.generation_config.audio_chunk_duration = INTERNAL_CHUNK_SECONDS;
    request.generation_config.audio_chunk_threshold = INTERNAL_CHUNK_SECONDS;

    let audio = pipeline
        .generate(&request)
        .map_err(map_omnivoice_error)?
        .into_iter()
        .next()
        .ok_or_else(|| {
            AppError::SpeechEngineUnavailable("OmniVoice nao gerou audio.".to_string())
        })?;

    Ok(DecodedAudio::new(
        trim_segment_silence(audio.samples),
        audio.sample_rate,
    ))
}

#[cfg(feature = "ml")]
fn load_pipeline(model_dir: PathBuf) -> AppResult<Phase3Pipeline> {
    #[cfg(not(feature = "cuda"))]
    {
        let _ = model_dir;
        Err(AppError::SpeechEngineUnavailable(
            "OmniVoice requer GPU; compile com --features cuda para habilitar Candle CUDA."
                .to_string(),
        ))
    }

    #[cfg(feature = "cuda")]
    {
        let options = RuntimeOptions::new(model_dir)
            .with_device(DeviceSpec::Cuda(0))
            .with_dtype(DTypeSpec::F16);
        Phase3Pipeline::from_options(options).map_err(map_omnivoice_error)
    }
}

#[cfg(feature = "ml")]
async fn load_shared_pipeline(model_dir: PathBuf) -> AppResult<SharedPhase3Pipeline> {
    tauri::async_runtime::spawn_blocking(move || {
        load_pipeline(model_dir).map(|pipeline| Arc::new(Mutex::new(pipeline)))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[cfg(feature = "ml")]
fn generation_request(
    text: String,
    voice_clone_prompt: Option<&VoiceClonePrompt>,
    target_duration_seconds: Option<f32>,
    options: &DubbingOptions,
    settings: &NativeSynthesisSettings,
) -> GenerationRequest {
    let mut request = GenerationRequest::new_text_only(text);
    request.languages = vec![language_for_omnivoice(options.target_language)];
    request.voice_clone_prompts = vec![voice_clone_prompt.cloned()];
    request.instructs = vec![design_instruct(settings)];
    request.durations = vec![target_duration_seconds];
    request.speeds = vec![target_duration_seconds
        .is_none()
        .then_some(settings.speed)
        .flatten()];
    request.generation_config.num_step = settings.num_step as usize;
    request.generation_config.guidance_scale = settings.guidance_scale;
    request.generation_config.position_temperature = if options.omni_temperature > 0.0
        && (settings.position_temperature - NativeSynthesisSettings::default().position_temperature)
            .abs()
            <= f32::EPSILON
    {
        options.omni_temperature
    } else {
        settings.position_temperature
    };
    request.generation_config.class_temperature = settings.class_temperature;
    request.generation_config.preprocess_prompt = settings.preprocess_prompt;
    request.generation_config.postprocess_output = settings.postprocess_output;
    request.generation_config.denoise = settings.denoise;
    request
}

#[cfg(feature = "ml")]
fn design_instruct(settings: &NativeSynthesisSettings) -> Option<String> {
    if !matches!(settings.voice_mode, VoiceMode::Design) {
        return None;
    }

    settings
        .instruct
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(feature = "ml")]
fn prepare_short_reference(
    reference_audio: &Path,
    source_text: &str,
    timing: AudioTimingProfile,
) -> AppResult<ShortReferencePrompt> {
    let decoded = decode_audio_mono_f32(reference_audio)?;
    let peak = decoded
        .samples
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()));
    let active_start = active_sample_range(&decoded.samples, peak)
        .map(|(start, _)| start)
        .unwrap_or(0);
    let target_samples = seconds_to_samples(REFERENCE_TARGET_SECONDS, decoded.sample_rate);
    let max_samples = seconds_to_samples(REFERENCE_MAX_SECONDS, decoded.sample_rate);
    let min_samples = seconds_to_samples(REFERENCE_MIN_SECONDS, decoded.sample_rate);

    let mut end = active_start
        .saturating_add(target_samples)
        .min(decoded.samples.len());
    if end.saturating_sub(active_start) < min_samples {
        end = active_start
            .saturating_add(min_samples)
            .min(decoded.samples.len());
    }
    end = active_start
        .saturating_add(max_samples)
        .min(end)
        .min(decoded.samples.len());

    let samples = if active_start < end {
        decoded.samples[active_start..end].to_vec()
    } else {
        decoded
            .samples
            .iter()
            .copied()
            .take(target_samples.min(decoded.samples.len()))
            .collect()
    };
    if samples.is_empty() {
        return Err(AppError::SpeechEngineUnavailable(
            "nao foi possivel extrair referencia curta para OmniVoice".to_string(),
        ));
    }

    let duration_seconds = samples.len() as f32 / decoded.sample_rate as f32;
    let text = reference_text_excerpt(
        source_text,
        duration_seconds,
        timing
            .target_voice_duration_seconds()
            .unwrap_or(duration_seconds),
    );

    Ok(ShortReferencePrompt {
        audio: ReferenceAudioInput::Waveform(WaveformInput::mono(samples, decoded.sample_rate)),
        text,
    })
}

#[cfg(feature = "ml")]
fn reference_text_excerpt(
    source_text: &str,
    reference_seconds: f32,
    source_voice_seconds: f32,
) -> String {
    let words = source_text.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return String::new();
    }

    let ratio = if source_voice_seconds > f32::EPSILON {
        (reference_seconds / source_voice_seconds).clamp(0.03, 1.0)
    } else {
        1.0
    };
    let word_count =
        ((words.len() as f32 * ratio).ceil() as usize).clamp(4.min(words.len()), words.len());
    words[..word_count].join(" ")
}

#[cfg(feature = "ml")]
fn seconds_to_samples(seconds: f32, sample_rate: u32) -> usize {
    (seconds.max(0.0) * sample_rate as f32).round().max(1.0) as usize
}

#[cfg(feature = "ml")]
fn synthesis_segments(text: &str, target_duration_seconds: Option<f32>) -> Vec<String> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Vec::new();
    }

    let total_chars = normalized.chars().count();
    let duration_segments = target_duration_seconds
        .map(|duration| (duration / SYNTHESIS_SEGMENT_TARGET_SECONDS).ceil() as usize)
        .unwrap_or(1);
    let target_segments = duration_segments.max(total_chars.div_ceil(SYNTHESIS_SEGMENT_MAX_CHARS));
    let target_chars = total_chars
        .div_ceil(target_segments.max(1))
        .clamp(SYNTHESIS_SEGMENT_MIN_CHARS, SYNTHESIS_SEGMENT_MAX_CHARS);

    let mut segments = Vec::new();
    let mut current = String::new();
    for unit in sentence_units(&normalized) {
        for piece in split_long_unit(&unit, target_chars) {
            let current_chars = current.chars().count();
            let piece_chars = piece.chars().count();
            if !current.is_empty() && current_chars + 1 + piece_chars > target_chars {
                segments.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(&piece);
        }
    }

    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

#[cfg(feature = "ml")]
fn sentence_units(text: &str) -> Vec<String> {
    let mut units = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        current.push(character);
        if matches!(character, '.' | '!' | '?' | ';' | ':') {
            let unit = current.trim();
            if !unit.is_empty() {
                units.push(unit.to_string());
            }
            current.clear();
        }
    }

    let tail = current.trim();
    if !tail.is_empty() {
        units.push(tail.to_string());
    }
    units
}

#[cfg(feature = "ml")]
fn split_long_unit(unit: &str, target_chars: usize) -> Vec<String> {
    if unit.chars().count() <= target_chars {
        return vec![unit.to_string()];
    }

    let mut pieces = Vec::new();
    let mut current = String::new();
    for word in unit.split_whitespace() {
        let current_chars = current.chars().count();
        let word_chars = word.chars().count();
        if !current.is_empty() && current_chars + 1 + word_chars > target_chars {
            pieces.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }

    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
}

#[cfg(feature = "ml")]
fn segment_durations(segments: &[String], total_duration: Option<f32>) -> Vec<Option<f32>> {
    let Some(total_duration) = total_duration else {
        return vec![None; segments.len()];
    };
    let total_chars = segments
        .iter()
        .map(|segment| segment.chars().count())
        .sum::<usize>()
        .max(1);

    segments
        .iter()
        .map(|segment| {
            let ratio = segment.chars().count() as f32 / total_chars as f32;
            Some((total_duration * ratio).max(0.75))
        })
        .collect()
}

#[cfg(feature = "ml")]
fn concatenate_segments(segments: Vec<DecodedAudio>) -> AppResult<DecodedAudio> {
    let mut segments = segments.into_iter();
    let Some(first) = segments.next() else {
        return Err(AppError::SpeechEngineUnavailable(
            "OmniVoice nao gerou segmentos de audio.".to_string(),
        ));
    };

    let sample_rate = first.sample_rate;
    let mut samples = first.samples;
    for segment in segments {
        if segment.sample_rate != sample_rate {
            return Err(AppError::SpeechEngineUnavailable(format!(
                "OmniVoice retornou sample rates inconsistentes: {} e {}",
                sample_rate, segment.sample_rate
            )));
        }
        samples.extend(segment.samples.into_iter().map(sanitize_sample));
    }

    Ok(DecodedAudio::new(samples, sample_rate))
}

#[cfg(feature = "ml")]
fn apply_segment_audio_polish(
    mut audio: DecodedAudio,
    settings: &NativeSynthesisSettings,
) -> DecodedAudio {
    reduce_sibilance(
        &mut audio.samples,
        audio.sample_rate,
        settings.sibilance_reduction,
    );
    reduce_metallic_artifacts(
        &mut audio.samples,
        audio.sample_rate,
        settings.artifact_reduction,
    );
    audio
}

#[cfg(feature = "ml")]
fn apply_original_timing(
    audio: DecodedAudio,
    timing: AudioTimingProfile,
    level_settings: AudioLevelSettings,
) -> DecodedAudio {
    let sample_rate = audio.sample_rate;
    let mut voice_samples = trim_leading_silence(audio.samples);
    if level_settings.match_source_loudness {
        match_source_loudness(
            &mut voice_samples,
            timing.rms_amplitude,
            timing.peak_amplitude * 0.98,
            level_settings.loudness_match_strength,
        );
    } else {
        normalize_peak(&mut voice_samples, timing.peak_amplitude * 0.96);
    }
    apply_output_gain(&mut voice_samples, level_settings.output_gain_db, 0.98);

    let leading = ms_to_samples(timing.leading_silence_ms, sample_rate);
    let trailing = ms_to_samples(timing.trailing_silence_ms, sample_rate);
    let minimum = ms_to_samples(timing.total_ms, sample_rate);
    let mut samples = Vec::with_capacity(leading + voice_samples.len() + trailing);
    samples.resize(leading, 0.0);
    samples.extend(voice_samples.into_iter().map(sanitize_sample));
    samples.resize(samples.len() + trailing, 0.0);
    if samples.len() < minimum {
        samples.resize(minimum, 0.0);
    }

    DecodedAudio::new(samples, sample_rate)
}

#[cfg(feature = "ml")]
fn match_source_loudness(samples: &mut [f32], target_rms: f32, peak_limit: f32, strength: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= f32::EPSILON {
        limit_peak(samples, peak_limit);
        return;
    }

    let current_rms = rms_level(samples);
    if current_rms <= f32::EPSILON {
        return;
    }

    let target_rms = target_rms.clamp(0.01, peak_limit.max(0.02) * 0.82);
    let desired_gain = target_rms / current_rms;
    let blended_gain = 1.0 + (desired_gain - 1.0) * strength;
    apply_gain_with_peak_guard(samples, blended_gain, peak_limit);
}

#[cfg(feature = "ml")]
fn apply_output_gain(samples: &mut [f32], gain_db: f32, peak_limit: f32) {
    if gain_db.abs() <= f32::EPSILON {
        limit_peak(samples, peak_limit);
        return;
    }

    let gain = 10_f32.powf(gain_db.clamp(-12.0, 12.0) / 20.0);
    apply_gain_with_peak_guard(samples, gain, peak_limit);
}

#[cfg(feature = "ml")]
fn apply_gain_with_peak_guard(samples: &mut [f32], gain: f32, peak_limit: f32) {
    if samples.is_empty() || !gain.is_finite() {
        return;
    }

    let peak = peak_level(samples);
    if peak <= f32::EPSILON {
        return;
    }

    let peak_limit = peak_limit.clamp(0.05, 0.98);
    let guarded_gain = if gain > 1.0 {
        gain.min(peak_limit / peak)
    } else {
        gain
    };
    for sample in &mut *samples {
        *sample = sanitize_sample(*sample * guarded_gain);
    }
    limit_peak(samples, peak_limit);
}

#[cfg(feature = "ml")]
fn limit_peak(samples: &mut [f32], peak_limit: f32) {
    let peak = peak_level(samples);
    let peak_limit = peak_limit.clamp(0.05, 0.98);
    if peak <= peak_limit || peak <= f32::EPSILON {
        return;
    }

    let gain = peak_limit / peak;
    for sample in samples {
        *sample = sanitize_sample(*sample * gain);
    }
}

#[cfg(feature = "ml")]
fn reduce_sibilance(samples: &mut [f32], sample_rate: u32, strength: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if samples.is_empty() || sample_rate == 0 || strength <= f32::EPSILON {
        return;
    }

    let highpass_alpha = one_pole_highpass_alpha(4_500.0, sample_rate);
    let lowpass_alpha = one_pole_lowpass_alpha(11_000.0, sample_rate);
    let threshold = (rms_level(samples) * (1.35 - 0.55 * strength)).max(0.006);
    let attack = smoothing_coefficient(0.003, sample_rate);
    let release = smoothing_coefficient(0.045, sample_rate);
    let mut previous_input = 0.0;
    let mut highpassed = 0.0;
    let mut bandpassed = 0.0;
    let mut envelope = 0.0;

    for sample in samples {
        let input = *sample;
        highpassed = highpass_alpha * (highpassed + input - previous_input);
        previous_input = input;
        bandpassed += lowpass_alpha * (highpassed - bandpassed);

        let magnitude = bandpassed.abs();
        let coefficient = if magnitude > envelope {
            attack
        } else {
            release
        };
        envelope = coefficient * envelope + (1.0 - coefficient) * magnitude;
        let over_threshold = if envelope > threshold {
            (envelope - threshold) / envelope
        } else {
            0.0
        };
        let reduction = over_threshold * strength;
        *sample = sanitize_sample(input - bandpassed * reduction);
    }
}

#[cfg(feature = "ml")]
fn reduce_metallic_artifacts(samples: &mut [f32], sample_rate: u32, strength: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if samples.is_empty() || sample_rate == 0 || strength <= f32::EPSILON {
        return;
    }

    let cutoff = 12_000.0 - 8_500.0 * strength;
    let lowpass_alpha = one_pole_lowpass_alpha(cutoff, sample_rate);
    let high_damping = 0.78 * strength;
    let temporal_smoothing = 0.22 * strength;
    let mut lowpassed = 0.0;
    let mut previous_output = 0.0;

    for sample in samples {
        let input = *sample;
        lowpassed += lowpass_alpha * (input - lowpassed);
        let softened = lowpassed + (input - lowpassed) * (1.0 - high_damping);
        let output = softened * (1.0 - temporal_smoothing) + previous_output * temporal_smoothing;
        previous_output = output;
        *sample = sanitize_sample(output);
    }
}

#[cfg(feature = "ml")]
fn peak_level(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()))
}

#[cfg(feature = "ml")]
fn rms_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

#[cfg(feature = "ml")]
fn one_pole_lowpass_alpha(cutoff_hz: f32, sample_rate: u32) -> f32 {
    let nyquist = sample_rate as f32 * 0.5;
    let cutoff_hz = cutoff_hz.clamp(20.0, (nyquist * 0.92).max(20.0));
    let dt = 1.0 / sample_rate as f32;
    let rc = 1.0 / (std::f32::consts::TAU * cutoff_hz);
    dt / (rc + dt)
}

#[cfg(feature = "ml")]
fn one_pole_highpass_alpha(cutoff_hz: f32, sample_rate: u32) -> f32 {
    let nyquist = sample_rate as f32 * 0.5;
    let cutoff_hz = cutoff_hz.clamp(20.0, (nyquist * 0.92).max(20.0));
    let dt = 1.0 / sample_rate as f32;
    let rc = 1.0 / (std::f32::consts::TAU * cutoff_hz);
    rc / (rc + dt)
}

#[cfg(feature = "ml")]
fn smoothing_coefficient(time_seconds: f32, sample_rate: u32) -> f32 {
    (-1.0 / (time_seconds * sample_rate as f32)).exp()
}

#[cfg(feature = "ml")]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[cfg(feature = "ml")]
fn non_empty_str(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

#[cfg(feature = "ml")]
fn trim_leading_silence(samples: Vec<f32>) -> Vec<f32> {
    let peak = samples
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()));
    active_sample_range(&samples, peak)
        .map(|(start, _)| samples[start..].to_vec())
        .unwrap_or(samples)
}

#[cfg(feature = "ml")]
fn trim_segment_silence(samples: Vec<f32>) -> Vec<f32> {
    let peak = samples
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()));
    active_sample_range(&samples, peak)
        .map(|(start, end)| samples[start..=end].to_vec())
        .unwrap_or(samples)
}

#[cfg(feature = "ml")]
fn normalize_peak(samples: &mut [f32], target_peak: f32) {
    let peak = samples
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()));
    if peak <= f32::EPSILON {
        return;
    }

    let gain = target_peak.clamp(0.05, 0.98) / peak;
    for sample in samples {
        *sample = sanitize_sample(*sample * gain);
    }
}

#[cfg(feature = "ml")]
fn sanitize_sample(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-0.999, 0.999)
    } else {
        0.0
    }
}

#[cfg(feature = "ml")]
fn language_for_omnivoice(language: LanguageCode) -> Option<String> {
    language.as_bcp47().map(str::to_string)
}

#[cfg(feature = "ml")]
fn map_omnivoice_error(error: OmniVoiceError) -> AppError {
    AppError::SpeechEngineUnavailable(error.to_string())
}

#[cfg(all(test, feature = "ml"))]
mod tests {
    use super::*;

    #[test]
    fn synthesis_segments_follow_duration_budget() {
        let text = "A luz brilha nas trevas. O verbo se fez carne e habitou entre nos. Um anjo apareceu aos pastores e anunciou boas noticias para todo o povo.";

        let segments = synthesis_segments(text, Some(32.0));

        assert!(segments.len() >= 4);
        assert!(segments.iter().all(|segment| !segment.trim().is_empty()));
        assert!(segments
            .iter()
            .all(|segment| segment.chars().count() <= 180));
    }

    #[test]
    fn whole_file_synthesis_keeps_text_in_one_omnivoice_request() {
        let request = OwnedSynthesisRequest {
            text: "Primeira frase. Segunda frase longa para simular um arquivo maior.".to_string(),
            source_audio: PathBuf::from("source.wav"),
            reference_audio: PathBuf::from("source.wav"),
            reference_text: "Original first sentence. Original second sentence.".to_string(),
            output_path: PathBuf::from("out.wav"),
            options: DubbingOptions::default(),
            line_overrides: Vec::new(),
            hooks: SynthesisHooks::default(),
        };

        let segments = synthesis_segments_for_request(&request, Some(32.0)).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].duration_seconds, Some(32.0));
        assert_eq!(
            segments[0].text,
            "Primeira frase. Segunda frase longa para simular um arquivo maior."
        );
    }

    #[test]
    fn segment_durations_preserve_total_shape() {
        let segments = vec!["curto".to_string(), "um trecho um pouco maior".to_string()];

        let durations = segment_durations(&segments, Some(12.0));

        assert_eq!(durations.len(), 2);
        assert!(durations[1].expect("long duration") > durations[0].expect("short duration"));
    }

    #[test]
    fn reference_text_excerpt_matches_short_reference_window() {
        let text = "one two three four five six seven eight nine ten";

        let excerpt = reference_text_excerpt(text, 8.0, 40.0);

        assert_eq!(excerpt, "one two three four");
    }

    #[test]
    fn generation_request_maps_design_settings_without_clone_prompt() {
        let options = DubbingOptions::default();
        let settings = NativeSynthesisSettings {
            voice_mode: VoiceMode::Design,
            instruct: Some("female, young adult, high pitch".to_string()),
            speed: Some(1.25),
            duration_seconds: None,
            num_step: 32,
            guidance_scale: 2.5,
            position_temperature: 1.5,
            class_temperature: 0.2,
            denoise: false,
            preprocess_prompt: false,
            postprocess_output: false,
            ..NativeSynthesisSettings::default()
        };

        let request = generation_request("Ola".to_string(), None, None, &options, &settings);

        assert_eq!(
            request.instructs[0].as_deref(),
            Some("female, young adult, high pitch")
        );
        assert!(request.voice_clone_prompts[0].is_none());
        assert_eq!(request.speeds[0], Some(1.25));
        assert_eq!(request.generation_config.num_step, 32);
        assert!(!request.generation_config.denoise);
    }

    #[test]
    fn generation_request_prefers_duration_over_speed() {
        let options = DubbingOptions::default();
        let settings = NativeSynthesisSettings {
            speed: Some(1.5),
            ..NativeSynthesisSettings::default()
        };
        let prompt = VoiceClonePrompt::new_empty("referencia");

        let request = generation_request(
            "Ola".to_string(),
            Some(&prompt),
            Some(2.5),
            &options,
            &settings,
        );

        assert!(request.voice_clone_prompts[0].is_some());
        assert_eq!(request.durations[0], Some(2.5));
        assert_eq!(request.speeds[0], None);
    }

    #[test]
    fn generation_request_preserves_native_tags_for_omnivoice_frontend() {
        let options = DubbingOptions::default();
        let settings = NativeSynthesisSettings::default();

        let request = generation_request(
            "[sigh] Ola [question-ah]?".to_string(),
            None,
            None,
            &options,
            &settings,
        );

        assert_eq!(request.texts[0], "[sigh] Ola [question-ah]?");
    }

    #[test]
    fn line_metadata_tags_are_applied_to_synthesis_text() {
        assert_eq!(
            tagged_synthesis_text(&["[sigh]".to_string()], "Ola mundo."),
            "[sigh] Ola mundo."
        );
        assert_eq!(
            tagged_synthesis_text(
                &["[sigh]".to_string(), "[sigh]".to_string()],
                "[sigh] Ola mundo."
            ),
            "[sigh] Ola mundo."
        );
    }

    #[test]
    fn audio_polish_matches_source_loudness_without_clipping() {
        let timing = AudioTimingProfile {
            total_ms: 100,
            leading_silence_ms: 0,
            trailing_silence_ms: 0,
            voice_ms: 100,
            peak_amplitude: 0.4,
            rms_amplitude: 0.18,
        };
        let audio = DecodedAudio::new(vec![0.04; 100], 1_000);

        let polished = apply_original_timing(
            audio,
            timing,
            AudioLevelSettings {
                match_source_loudness: true,
                loudness_match_strength: 1.0,
                output_gain_db: 0.0,
            },
        );

        assert!(rms_level(&polished.samples) > 0.15);
        assert!(peak_level(&polished.samples) <= 0.4);
    }

    #[test]
    fn audio_polish_output_gain_can_reduce_final_level() {
        let timing = AudioTimingProfile {
            total_ms: 100,
            leading_silence_ms: 0,
            trailing_silence_ms: 0,
            voice_ms: 100,
            peak_amplitude: 0.6,
            rms_amplitude: 0.3,
        };
        let audio = DecodedAudio::new(vec![0.4; 100], 1_000);

        let polished = apply_original_timing(
            audio,
            timing,
            AudioLevelSettings {
                match_source_loudness: false,
                loudness_match_strength: 0.0,
                output_gain_db: -6.0,
            },
        );

        assert!(rms_level(&polished.samples) < 0.31);
        assert!(peak_level(&polished.samples) < 0.31);
    }
}
