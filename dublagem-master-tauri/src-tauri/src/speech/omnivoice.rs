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
use dublagem_domain::DubbingOptions;
#[cfg(feature = "ml")]
use dublagem_domain::LanguageCode;
#[cfg(feature = "ml")]
use omnivoice_infer::{
    contracts::{DecodedAudio, GenerationRequest, ReferenceAudioInput, WaveformInput},
    pipeline::Phase3Pipeline,
    DTypeSpec, DeviceSpec, OmniVoiceError, RuntimeOptions,
};
use std::path::{Path, PathBuf};

#[cfg(feature = "ml")]
const SYNTHESIS_SEGMENT_TARGET_SECONDS: f32 = 8.0;
#[cfg(feature = "ml")]
const SYNTHESIS_SEGMENT_MIN_CHARS: usize = 24;
#[cfg(feature = "ml")]
const SYNTHESIS_SEGMENT_MAX_CHARS: usize = 180;
#[cfg(feature = "ml")]
const SYNTHESIS_NUM_STEPS: usize = 48;
#[cfg(feature = "ml")]
const INTERNAL_CHUNK_SECONDS: f32 = 12.0;
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
struct OwnedSynthesisRequest {
    text: String,
    source_audio: PathBuf,
    reference_audio: PathBuf,
    reference_text: String,
    output_path: PathBuf,
    options: DubbingOptions,
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
            hooks: request.hooks,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OmniVoiceCandleSynthesizer {
    model_dir: Option<PathBuf>,
}

impl OmniVoiceCandleSynthesizer {
    pub fn new(model_dir: Option<PathBuf>) -> Self {
        Self { model_dir }
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
        generate_pool_with_model(model_dir, output_dir).await
    }
}

#[cfg(feature = "ml")]
async fn synthesize_with_model(model_dir: &Path, request: SynthesisRequest<'_>) -> AppResult<()> {
    let model_dir = model_dir.to_path_buf();
    let request = OwnedSynthesisRequest::from_request(request);

    tauri::async_runtime::spawn_blocking(move || synthesize_blocking(model_dir, request))
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
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
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
fn synthesize_blocking(model_dir: PathBuf, request: OwnedSynthesisRequest) -> AppResult<()> {
    let timing = audio_timing_profile(&request.source_audio, request.options.pad_ms)?;
    let target_duration = timing.target_voice_duration_seconds();
    let segments = synthesis_segments(&request.text, target_duration);
    if segments.is_empty() {
        return Err(AppError::InvalidConfig(
            "texto destino vazio; nao ha conteudo para sintese".to_string(),
        ));
    }
    request.hooks.report(0, segments.len());

    let pipeline = load_pipeline(model_dir)?;
    let short_reference =
        prepare_short_reference(&request.reference_audio, &request.reference_text, timing)?;
    let durations = segment_durations(&segments, target_duration);
    let mut segment_audio = Vec::with_capacity(segments.len());

    for (index, segment) in segments.iter().enumerate() {
        if request.hooks.is_cancelled() {
            return Err(AppError::Internal("sintese cancelada".to_string()));
        }

        let audio = synthesize_segment(
            &pipeline,
            segment,
            durations[index],
            &short_reference.audio,
            &short_reference.text,
            request.options,
        )?;
        segment_audio.push(audio);
        request.hooks.report(index + 1, segments.len());
    }

    let audio = concatenate_segments(segment_audio)?;
    let audio = apply_original_timing(audio, timing);

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
fn synthesize_segment(
    pipeline: &Phase3Pipeline,
    text: &str,
    target_duration_seconds: Option<f32>,
    reference_audio: &ReferenceAudioInput,
    reference_text: &str,
    options: DubbingOptions,
) -> AppResult<DecodedAudio> {
    let mut request = generation_request(
        text.to_string(),
        reference_audio,
        reference_text,
        target_duration_seconds,
        options,
        SYNTHESIS_NUM_STEPS,
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
fn generation_request(
    text: String,
    reference_audio: &ReferenceAudioInput,
    reference_text: &str,
    target_duration_seconds: Option<f32>,
    options: DubbingOptions,
    num_step: usize,
) -> GenerationRequest {
    let mut request = GenerationRequest::new_text_only(text);
    request.languages = vec![language_for_omnivoice(options.target_language)];
    request.ref_audios = vec![Some(reference_audio.clone())];
    request.ref_texts = vec![non_empty_string(reference_text)];
    request.durations = vec![target_duration_seconds];
    request.generation_config.num_step = num_step;
    request.generation_config.guidance_scale = 2.0;
    request.generation_config.position_temperature = if options.omni_temperature > 0.0 {
        options.omni_temperature
    } else {
        1.0
    };
    request.generation_config.class_temperature = 0.0;
    request.generation_config.preprocess_prompt = true;
    request.generation_config.postprocess_output = true;
    request.generation_config.denoise = true;
    request
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
fn apply_original_timing(audio: DecodedAudio, timing: AudioTimingProfile) -> DecodedAudio {
    let sample_rate = audio.sample_rate;
    let mut voice_samples = trim_leading_silence(audio.samples);
    normalize_peak(&mut voice_samples, timing.peak_amplitude * 0.96);

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
fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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
    fn segment_durations_preserve_total_shape() {
        let segments = vec!["curto".to_string(), "um trecho um pouco maior".to_string()];

        let durations = segment_durations(&segments, Some(12.0));

        assert_eq!(durations.len(), 2);
        assert!(durations[1].expect("long duration") > durations[0].expect("short duration"));
    }
}
