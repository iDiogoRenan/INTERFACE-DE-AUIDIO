#[cfg(feature = "ml")]
use super::SynthesisHooks;
use super::{ptbr_voice_profiles, SynthesisRequest, VoiceSynthesizer};
#[cfg(feature = "ml")]
use crate::audio::{
    active_sample_range, audio_timing_profile, decode_audio_mono_f32, ms_to_samples,
    short_reference_waveform, speech_windows, write_pcm16_wav_mono, AudioTimingProfile,
    DecodedAudio as SourceDecodedAudio, SpeechWindow,
};
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use dublagem_domain::TimingAlignmentReport;
#[cfg(feature = "ml")]
use dublagem_domain::{
    ChunkLimitPolicy, DubbingOptions, LanguageCode, LineSynthesisOverride, NativeSynthesisSettings,
    SpeechModelId, TimingAdjustmentAction, TimingAlignmentChunkReport, TimingChunkStatus,
    VoiceMode, OMNIVOICE_MAX_SYNTHESIS_SECONDS,
};
#[cfg(feature = "ml")]
use omnivoice_infer::{
    contracts::{
        DecodedAudio, GenerationRequest, ReferenceAudioInput, VoiceClonePrompt, WaveformInput,
    },
    pipeline::Phase3Pipeline,
    OmniVoiceError,
};
#[cfg(feature = "cuda")]
use omnivoice_infer::{DTypeSpec, DeviceSpec, RuntimeOptions};
use std::path::{Path, PathBuf};
#[cfg(feature = "ml")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "ml")]
const SEGMENT_CROSSFADE_MS: u32 = 35;
#[cfg(feature = "ml")]
const MAX_TEMPORAL_CHUNK_SECONDS: f32 = 12.0;
#[cfg(feature = "ml")]
const MIN_TEMPORAL_CHUNK_SECONDS: f32 = 1.0;

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
    source_text: String,
    source_audio: PathBuf,
    reference_audio: PathBuf,
    reference_text: String,
    output_path: PathBuf,
    options: DubbingOptions,
    pinned_tags: Vec<String>,
    line_overrides: Vec<LineSynthesisOverride>,
    hooks: SynthesisHooks,
}

#[cfg(feature = "ml")]
impl OwnedSynthesisRequest {
    fn from_request(request: SynthesisRequest<'_>) -> Self {
        Self {
            text: request.text.to_string(),
            source_text: request.source_text.to_string(),
            source_audio: request.source_audio.to_path_buf(),
            reference_audio: request.reference_audio.to_path_buf(),
            reference_text: request.reference_text.to_string(),
            output_path: request.output_path.to_path_buf(),
            options: request.options,
            pinned_tags: request.pinned_tags.to_vec(),
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
        #[cfg(feature = "cuda")]
        crate::speech::gpu::require_cuda_gpu()?;

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
    async fn synthesize(&self, request: SynthesisRequest<'_>) -> AppResult<TimingAlignmentReport> {
        let Some(model_dir) = &self.model_dir else {
            return Err(AppError::SpeechEngineUnavailable(
                "pasta de modelos não configurada. Selecione a pasta em Ajustes antes de dublar."
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
                "pasta de modelos não configurada. Selecione a pasta em Ajustes antes de dublar."
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
async fn synthesize_with_model(
    model_dir: &Path,
    request: SynthesisRequest<'_>,
) -> AppResult<TimingAlignmentReport> {
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
) -> AppResult<TimingAlignmentReport> {
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
async fn synthesize_with_model(
    _model_dir: &Path,
    _request: SynthesisRequest<'_>,
) -> AppResult<TimingAlignmentReport> {
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
                    "OmniVoice não gerou áudio para o perfil PT-BR.".to_string(),
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
) -> AppResult<TimingAlignmentReport> {
    let timing = audio_timing_profile(&request.source_audio, request.options.pad_ms)?;
    let total_duration_seconds = timing.total_ms as f32 / 1000.0;
    let segments = synthesis_segments_for_request(&request, timing)?;
    if segments.is_empty() {
        return Err(AppError::InvalidConfig(
            "texto destino vazio; não há conteúdo para síntese".to_string(),
        ));
    }
    request.hooks.report(0, segments.len());
    let voice_clone_prompt = if segments
        .iter()
        .any(|segment| matches!(segment.settings.voice_mode, VoiceMode::Clone))
    {
        let short_reference =
            prepare_short_reference(&request.reference_audio, &request.reference_text)?;
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
    let chunk_limit_exceeded =
        segments.len() > request.options.max_synthesis_chunks.max(1) as usize;
    let processed_in_batches = chunk_limit_exceeded
        && matches!(
            request.options.timing_alignment.chunk_limit_policy,
            ChunkLimitPolicy::ProcessInBatches
        );
    let mut placed_segments = Vec::with_capacity(segments.len());
    let mut chunk_reports = Vec::with_capacity(segments.len());
    let source_audio = decode_audio_mono_f32(&request.source_audio)?;
    let artifact_dir = alignment_artifact_dir(&request.output_path);
    std::fs::create_dir_all(&artifact_dir)?;
    let artifact_context = SegmentArtifactContext {
        dir: artifact_dir.as_path(),
        source_audio: &source_audio,
    };

    for (index, segment) in segments.iter().enumerate() {
        if request.hooks.is_cancelled() {
            return Err(AppError::Internal("síntese cancelada".to_string()));
        }

        let result = synthesize_aligned_segment(
            pipeline,
            &request.options,
            segment,
            voice_clone_prompt_for(&segment.settings, voice_clone_prompt.as_ref())?,
            chunk_limit_exceeded,
            processed_in_batches,
            artifact_context,
        )?;
        placed_segments.push(PlacedTimelineSegment {
            start_seconds: segment.start_seconds,
            audio: result.audio,
        });
        chunk_reports.push(result.report);
        request.hooks.report(index + 1, segments.len());
    }

    let level_settings = audio_level_settings(&segments);
    let audio = compose_timeline_audio(
        placed_segments,
        total_duration_seconds,
        timing,
        level_settings,
        &request.options,
    )?;

    if let Some(parent) = request
        .output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    audio
        .write_wav(&request.output_path)
        .map_err(map_omnivoice_error)?;

    let warnings = timing_alignment_warnings(
        segments.len(),
        request.options.max_synthesis_chunks,
        request.options.timing_alignment.chunk_limit_policy,
    );
    let has_critical_chunks = chunk_reports.iter().any(chunk_report_is_critical);
    Ok(TimingAlignmentReport {
        audio_id: request_audio_id(&request),
        file_name: request_file_name(&request),
        model_used: SpeechModelId::OmniVoice,
        total_chunks: segments.len(),
        configured_chunk_limit: request.options.max_synthesis_chunks,
        chunk_limit_policy: request.options.timing_alignment.chunk_limit_policy,
        chunk_limit_exceeded,
        processed_in_batches,
        has_critical_chunks,
        warnings,
        chunks: chunk_reports,
    })
}

#[cfg(feature = "ml")]
#[derive(Debug, Clone)]
struct SynthesisSegmentPlan {
    text: String,
    duration_seconds: Option<f32>,
    settings: NativeSynthesisSettings,
    original_duration_seconds: Option<f32>,
    audio_id: String,
    source_text: String,
    start_seconds: f32,
    end_seconds: f32,
    chunk_index: usize,
    total_chunks: usize,
}

#[cfg(feature = "ml")]
#[derive(Debug)]
struct AlignedSegmentResult {
    audio: DecodedAudio,
    report: TimingAlignmentChunkReport,
}

#[cfg(feature = "ml")]
struct PlacedTimelineSegment {
    start_seconds: f32,
    audio: DecodedAudio,
}

#[cfg(feature = "ml")]
#[derive(Clone, Copy)]
struct SegmentArtifactContext<'a> {
    dir: &'a Path,
    source_audio: &'a SourceDecodedAudio,
}

#[cfg(feature = "ml")]
struct SegmentArtifactPaths {
    original: PathBuf,
    dubbed: PathBuf,
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
    timing: AudioTimingProfile,
) -> AppResult<Vec<SynthesisSegmentPlan>> {
    let windows = source_speech_windows(request, timing)?;
    synthesis_segments_for_windows(request, windows)
}

#[cfg(feature = "ml")]
fn synthesis_segments_for_windows(
    request: &OwnedSynthesisRequest,
    windows: Vec<SpeechWindow>,
) -> AppResult<Vec<SynthesisSegmentPlan>> {
    let textual_segments = if request.line_overrides.is_empty() {
        global_synthesis_segments(request)
    } else {
        line_override_synthesis_segments(request)
    };
    let textual_segments = if textual_segments.is_empty() && !request.text.trim().is_empty() {
        global_synthesis_segments(request)
    } else {
        textual_segments
    };
    if textual_segments.is_empty() || windows.is_empty() {
        return Ok(Vec::new());
    }

    let target_count = alignment_chunk_count(request, &textual_segments, &windows);
    let windows = rebalance_speech_windows(windows, target_count);
    let target_texts = rebalance_text_units(
        textual_segments
            .iter()
            .map(|segment| segment.text.clone())
            .collect(),
        windows.len(),
    );
    let source_texts =
        rebalance_text_units(split_text_candidates(&request.source_text), windows.len());
    let total_chunks = windows.len();

    let mut segments = Vec::with_capacity(total_chunks);
    for (index, window) in windows.into_iter().enumerate() {
        let settings_index = scaled_source_index(index, textual_segments.len(), total_chunks);
        let settings = textual_segments
            .get(settings_index)
            .map(|segment| segment.settings.clone())
            .unwrap_or_else(|| request.options.native_synthesis.clone());
        let duration_seconds = window.duration_seconds();
        let mut segment = SynthesisSegmentPlan {
            text: naturalized_terminal_punctuation(
                target_texts
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or_default(),
            ),
            duration_seconds: Some(duration_seconds),
            settings,
            original_duration_seconds: Some(duration_seconds),
            audio_id: request_audio_id(request),
            source_text: source_texts.get(index).cloned().unwrap_or_default(),
            start_seconds: window.start_seconds,
            end_seconds: window.end_seconds,
            chunk_index: index + 1,
            total_chunks,
        };
        validate_omnivoice_duration(segment.duration_seconds)?;
        if segment.text.trim().is_empty() {
            segment.text = naturalized_terminal_punctuation(&request.text);
        }
        segments.push(segment);
    }

    Ok(segments)
}

#[cfg(feature = "ml")]
fn source_speech_windows(
    request: &OwnedSynthesisRequest,
    timing: AudioTimingProfile,
) -> AppResult<Vec<SpeechWindow>> {
    let detection = speech_windows(
        &request.source_audio,
        request.options.pad_ms,
        MAX_TEMPORAL_CHUNK_SECONDS,
    )?;
    if !detection.windows.is_empty() {
        return Ok(detection.windows);
    }

    let total_seconds = (timing.total_ms as f32 / 1000.0).max(MIN_TEMPORAL_CHUNK_SECONDS);
    let start_seconds = timing.leading_silence_ms as f32 / 1000.0;
    let end_seconds = (total_seconds - timing.trailing_silence_ms as f32 / 1000.0)
        .max(start_seconds + MIN_TEMPORAL_CHUNK_SECONDS)
        .min(total_seconds);
    Ok(vec![SpeechWindow {
        start_seconds,
        end_seconds,
    }])
}

#[cfg(feature = "ml")]
fn alignment_chunk_count(
    request: &OwnedSynthesisRequest,
    textual_segments: &[SynthesisSegmentPlan],
    windows: &[SpeechWindow],
) -> usize {
    if windows.is_empty() {
        return 0;
    }

    let minimum_chunks = minimum_chunks_for_duration(windows);
    let preferred_chunks = if request.line_overrides.is_empty() {
        textual_segments
            .first()
            .map(|segment| split_text_candidates_with_soft_boundaries(&segment.text).len())
            .unwrap_or(1)
            .max(1)
    } else {
        textual_segments.len().max(1)
    };

    preferred_chunks
        .max(minimum_chunks)
        .min(windows.len())
        .max(1)
}

#[cfg(feature = "ml")]
fn minimum_chunks_for_duration(windows: &[SpeechWindow]) -> usize {
    let (Some(first), Some(last)) = (windows.first(), windows.last()) else {
        return 0;
    };

    let span_seconds = (last.end_seconds - first.start_seconds).max(0.0);
    (span_seconds / OMNIVOICE_MAX_SYNTHESIS_SECONDS)
        .ceil()
        .max(1.0) as usize
}

#[cfg(feature = "ml")]
fn rebalance_speech_windows(windows: Vec<SpeechWindow>, target_count: usize) -> Vec<SpeechWindow> {
    if target_count == 0 || windows.len() <= target_count {
        return windows;
    }

    (0..target_count)
        .filter_map(|index| {
            let start = index * windows.len() / target_count;
            let end = if index + 1 == target_count {
                windows.len()
            } else {
                (index + 1) * windows.len() / target_count
            };
            let group = &windows[start..end.max(start + 1).min(windows.len())];
            let first = group.first()?;
            let last = group.last()?;
            Some(SpeechWindow {
                start_seconds: first.start_seconds,
                end_seconds: last.end_seconds,
            })
        })
        .collect()
}

#[cfg(feature = "ml")]
fn split_text_candidates(text: &str) -> Vec<String> {
    split_text_candidates_by_boundaries(text, false)
}

#[cfg(feature = "ml")]
fn split_text_candidates_with_soft_boundaries(text: &str) -> Vec<String> {
    split_text_candidates_by_boundaries(text, true)
}

#[cfg(feature = "ml")]
fn split_text_candidates_by_boundaries(text: &str, include_soft_boundaries: bool) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        current.push(character);
        if matches!(character, '.' | '!' | '?' | '…' | '\n')
            || (include_soft_boundaries && matches!(character, ',' | ';' | ':'))
        {
            push_text_candidate(&mut candidates, &mut current);
        }
    }
    push_text_candidate(&mut candidates, &mut current);
    if candidates.is_empty() && !text.trim().is_empty() {
        candidates.push(text.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    merge_tiny_text_units(candidates)
}

#[cfg(feature = "ml")]
fn push_text_candidate(candidates: &mut Vec<String>, current: &mut String) {
    let candidate = current.split_whitespace().collect::<Vec<_>>().join(" ");
    if !candidate.is_empty() {
        candidates.push(candidate);
    }
    current.clear();
}

#[cfg(feature = "ml")]
fn rebalance_text_units(mut units: Vec<String>, target_count: usize) -> Vec<String> {
    if target_count == 0 {
        return Vec::new();
    }
    units.retain(|unit| !unit.trim().is_empty());
    if units.is_empty() {
        return vec![String::new(); target_count];
    }
    if units.len() == target_count {
        return units;
    }
    if units.len() > target_count {
        return combine_text_units(units, target_count);
    }

    split_text_units(units.join(" "), target_count)
}

#[cfg(feature = "ml")]
fn combine_text_units(units: Vec<String>, target_count: usize) -> Vec<String> {
    (0..target_count)
        .map(|index| {
            let start = index * units.len() / target_count;
            let end = if index + 1 == target_count {
                units.len()
            } else {
                (index + 1) * units.len() / target_count
            };
            units[start..end.min(units.len())]
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

#[cfg(feature = "ml")]
fn split_text_units(text: String, target_count: usize) -> Vec<String> {
    let soft_units = split_text_candidates_with_soft_boundaries(&text);
    if soft_units.len() >= target_count {
        return combine_text_units(soft_units, target_count);
    }

    let words = text.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return vec![String::new(); target_count];
    }

    (0..target_count)
        .map(|index| {
            let start = index * words.len() / target_count;
            let end = if index + 1 == target_count {
                words.len()
            } else {
                (index + 1) * words.len() / target_count
            };
            words[start..end.min(words.len())].join(" ")
        })
        .collect()
}

#[cfg(feature = "ml")]
fn merge_tiny_text_units(units: Vec<String>) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    for unit in units {
        if word_count(&unit) < 3 && !merged.is_empty() {
            if let Some(previous) = merged.last_mut() {
                previous.push(' ');
                previous.push_str(unit.trim());
            }
            continue;
        }

        merged.push(unit);
    }

    if merged.len() > 1 && merged.last().is_some_and(|unit| word_count(unit) < 3) {
        if let Some(tail) = merged.pop() {
            if let Some(previous) = merged.last_mut() {
                previous.push(' ');
                previous.push_str(tail.trim());
            }
        }
    }

    if merged.len() > 1 && merged.first().is_some_and(|unit| word_count(unit) < 3) {
        let head = merged.remove(0);
        if let Some(first) = merged.first_mut() {
            *first = format!("{} {}", head.trim(), first.trim());
        }
    }

    merged
}

#[cfg(feature = "ml")]
fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

#[cfg(feature = "ml")]
fn scaled_source_index(index: usize, source_len: usize, target_len: usize) -> usize {
    if source_len <= 1 || target_len == 0 {
        return 0;
    }
    (index * source_len / target_len).min(source_len - 1)
}

#[cfg(feature = "ml")]
fn global_synthesis_segments(request: &OwnedSynthesisRequest) -> Vec<SynthesisSegmentPlan> {
    let text = naturalized_terminal_punctuation(&tagged_synthesis_text(
        &request.pinned_tags,
        &whole_file_synthesis_text(request),
    ));
    if text.is_empty() {
        return Vec::new();
    }

    vec![SynthesisSegmentPlan {
        text,
        duration_seconds: None,
        settings: request.options.native_synthesis.clone(),
        original_duration_seconds: None,
        audio_id: request_audio_id(request),
        source_text: request.source_text.clone(),
        start_seconds: 0.0,
        end_seconds: 0.0,
        chunk_index: 1,
        total_chunks: 1,
    }]
}

#[cfg(feature = "ml")]
fn line_override_synthesis_segments(request: &OwnedSynthesisRequest) -> Vec<SynthesisSegmentPlan> {
    let mut sorted = request
        .line_overrides
        .iter()
        .filter(|line| !line.target_text.trim().is_empty())
        .collect::<Vec<_>>();
    sorted.sort_by_key(|line| line.line_index);

    sorted
        .into_iter()
        .map(|line| {
            let tags = effective_synthesis_tags(&request.pinned_tags, &line.tags);
            let text = naturalized_terminal_punctuation(&tagged_synthesis_text(
                &tags,
                line.target_text.trim(),
            ));
            SynthesisSegmentPlan {
                text,
                duration_seconds: None,
                settings: line.settings.clone(),
                original_duration_seconds: None,
                audio_id: request_audio_id(request),
                source_text: String::new(),
                start_seconds: 0.0,
                end_seconds: 0.0,
                chunk_index: line.line_index + 1,
                total_chunks: 1,
            }
        })
        .filter(|segment| !segment.text.is_empty())
        .collect()
}

#[cfg(feature = "ml")]
fn whole_file_synthesis_text(request: &OwnedSynthesisRequest) -> String {
    request
        .text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
fn effective_synthesis_tags(pinned_tags: &[String], line_tags: &[String]) -> Vec<String> {
    let mut tags = Vec::new();
    for tag in pinned_tags.iter().chain(line_tags.iter()) {
        if dublagem_domain::OMNIVOICE_NATIVE_TAGS.contains(&tag.as_str()) && !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    tags
}

#[cfg(feature = "ml")]
fn naturalized_terminal_punctuation(text: &str) -> String {
    let trimmed = text.trim().trim_end_matches([',', ';', ':']).trim_end();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with('.')
        || trimmed.ends_with('!')
        || trimmed.ends_with('?')
        || trimmed.ends_with('…')
    {
        trimmed.to_string()
    } else {
        format!("{trimmed}.")
    }
}

#[cfg(feature = "ml")]
fn validate_omnivoice_duration(duration_seconds: Option<f32>) -> AppResult<()> {
    let Some(duration_seconds) = duration_seconds else {
        return Ok(());
    };
    if duration_seconds.is_finite() && duration_seconds > 0.0 {
        return Ok(());
    }

    Err(AppError::InvalidConfig(format!(
        "duração de síntese OmniVoice inválida: {duration_seconds:.2}s."
    )))
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
        AppError::SpeechEngineUnavailable("instrução de voz clonada indisponível".to_string())
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
    validate_omnivoice_duration(target_duration_seconds)?;
    let request = generation_request(
        text.to_string(),
        voice_clone_prompt,
        target_duration_seconds,
        options,
        settings,
    );

    let audio = pipeline
        .generate(&request)
        .map_err(map_omnivoice_error)?
        .into_iter()
        .next()
        .ok_or_else(|| {
            AppError::SpeechEngineUnavailable("OmniVoice não gerou áudio.".to_string())
        })?;

    Ok(DecodedAudio::new(audio.samples, audio.sample_rate))
}

#[cfg(feature = "ml")]
fn synthesize_aligned_segment(
    pipeline: &Phase3Pipeline,
    options: &DubbingOptions,
    segment: &SynthesisSegmentPlan,
    voice_clone_prompt: Option<&VoiceClonePrompt>,
    chunk_limit_exceeded: bool,
    processed_in_batches: bool,
    artifacts: SegmentArtifactContext<'_>,
) -> AppResult<AlignedSegmentResult> {
    let max_attempts = options.timing_alignment.max_regeneration_attempts.max(1);
    let mut text = segment.text.clone();
    let mut attempt = 1;

    loop {
        let audio = synthesize_segment(
            pipeline,
            &text,
            segment.duration_seconds,
            voice_clone_prompt,
            options,
            &segment.settings,
        )?;
        let audio = apply_segment_audio_polish(audio, &segment.settings);
        let fit = fit_segment_audio(audio, segment, options)?;
        let should_retry =
            fit.critical && options.timing_alignment.auto_text_adaptation && attempt < max_attempts;

        if should_retry {
            text = adapt_text_for_timing(&text, fit.generated_duration_seconds, segment, attempt);
            attempt += 1;
            continue;
        }

        let mut statuses = fit.statuses;
        let mut actions = fit.actions;
        if attempt > 1 {
            push_unique_status(&mut statuses, TimingChunkStatus::Regenerated);
            push_unique_action(&mut actions, TimingAdjustmentAction::Regenerated);
            push_unique_status(&mut statuses, TimingChunkStatus::TextAdapted);
            push_unique_action(&mut actions, TimingAdjustmentAction::TextAdapted);
        }
        if chunk_limit_exceeded {
            push_unique_status(&mut statuses, TimingChunkStatus::ChunkLimitExceeded);
        }
        if processed_in_batches {
            push_unique_status(&mut statuses, TimingChunkStatus::BatchProcessed);
            push_unique_action(&mut actions, TimingAdjustmentAction::BatchQueued);
        }
        let artifact_paths = write_segment_artifacts(segment, artifacts, &fit.audio)?;

        let report = TimingAlignmentChunkReport {
            segment_id: format!(
                "{}#{}",
                request_safe_stem(segment.audio_id.as_str()).unwrap_or("segment"),
                segment.chunk_index
            ),
            audio_id: segment.audio_id.clone(),
            chunk_index: segment.chunk_index,
            total_chunks: segment.total_chunks,
            start_original: f64::from(segment.start_seconds),
            end_original: f64::from(segment.end_seconds),
            duration_original: f64::from(
                segment
                    .original_duration_seconds
                    .or(segment.duration_seconds)
                    .unwrap_or_default(),
            ),
            texto_original_en: segment.source_text.clone(),
            texto_ptbr: text,
            original_segment_path: Some(artifact_paths.original),
            dubbed_segment_path: Some(artifact_paths.dubbed),
            duration_generated: Some(f64::from(fit.generated_duration_seconds)),
            duration_difference_percent: Some(fit.duration_difference_percent),
            statuses,
            actions_applied: actions,
            model_used: SpeechModelId::OmniVoice,
            attempts: attempt,
            failure_reason: fit.failure_reason,
            stretch_ratio: fit.stretch_ratio,
            overlap_seconds: None,
            abrupt_ending_detected: fit.abrupt_ending_detected,
        };

        return Ok(AlignedSegmentResult {
            audio: fit.audio,
            report,
        });
    }
}

#[cfg(feature = "ml")]
fn write_segment_artifacts(
    segment: &SynthesisSegmentPlan,
    artifacts: SegmentArtifactContext<'_>,
    dubbed_audio: &DecodedAudio,
) -> AppResult<SegmentArtifactPaths> {
    let original = segment_artifact_path(artifacts.dir, segment, "original");
    let dubbed = segment_artifact_path(artifacts.dir, segment, "dubbed");

    if let Some(parent) = original
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    write_source_segment_artifact(&original, segment, artifacts.source_audio)?;
    dubbed_audio
        .write_wav(&dubbed)
        .map_err(map_omnivoice_error)?;

    Ok(SegmentArtifactPaths { original, dubbed })
}

#[cfg(feature = "ml")]
fn write_source_segment_artifact(
    path: &Path,
    segment: &SynthesisSegmentPlan,
    source_audio: &SourceDecodedAudio,
) -> AppResult<()> {
    if source_audio.sample_rate == 0 {
        return Err(AppError::Internal(
            "segmento original sem taxa de amostragem".to_string(),
        ));
    }

    let sample_count = source_audio.samples.len();
    let start =
        seconds_to_sample_index(segment.start_seconds, source_audio.sample_rate).min(sample_count);
    let end = seconds_to_sample_index(segment.end_seconds, source_audio.sample_rate)
        .min(sample_count)
        .max(start);

    write_pcm16_wav_mono(
        path,
        source_audio.sample_rate,
        &source_audio.samples[start..end],
    )
}

#[cfg(feature = "ml")]
fn segment_artifact_path(
    artifact_dir: &Path,
    segment: &SynthesisSegmentPlan,
    suffix: &str,
) -> PathBuf {
    artifact_dir.join(format!(
        "{:04}-{}.wav",
        segment.chunk_index,
        sanitize_artifact_component(suffix)
    ))
}

#[cfg(feature = "ml")]
fn alignment_artifact_dir(output_path: &Path) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_artifact_component)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "audio".to_string());
    let parent = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    parent.join(format!("{stem}.alignment_chunks"))
}

#[cfg(feature = "ml")]
fn sanitize_artifact_component(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' => character,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[cfg(feature = "ml")]
fn seconds_to_sample_index(seconds: f32, sample_rate: u32) -> usize {
    (seconds.max(0.0) * sample_rate as f32).round() as usize
}

#[cfg(feature = "ml")]
#[derive(Debug)]
struct FittedSegmentAudio {
    audio: DecodedAudio,
    generated_duration_seconds: f32,
    duration_difference_percent: f32,
    statuses: Vec<TimingChunkStatus>,
    actions: Vec<TimingAdjustmentAction>,
    stretch_ratio: Option<f32>,
    failure_reason: Option<String>,
    critical: bool,
    abrupt_ending_detected: bool,
}

#[cfg(feature = "ml")]
fn fit_segment_audio(
    mut audio: DecodedAudio,
    segment: &SynthesisSegmentPlan,
    options: &DubbingOptions,
) -> AppResult<FittedSegmentAudio> {
    if audio.sample_rate == 0 {
        return Err(AppError::SpeechEngineUnavailable(
            "OmniVoice retornou áudio sem taxa de amostragem.".to_string(),
        ));
    }

    let original_duration_seconds = segment.duration_seconds.unwrap_or_else(|| {
        (segment.end_seconds - segment.start_seconds).max(MIN_TEMPORAL_CHUNK_SECONDS)
    });
    let generated_duration_seconds = audio.samples.len() as f32 / audio.sample_rate as f32;
    let duration_difference_percent =
        duration_difference_percent(generated_duration_seconds, original_duration_seconds);
    let mut statuses = Vec::new();
    let mut actions = Vec::new();
    let mut critical = false;
    let mut failure_reason = None;

    remove_dc_offset(&mut audio.samples);
    let mut samples = trim_leading_silence(audio.samples);
    if options.timing_alignment.normalize_loudness {
        normalize_peak(&mut samples, 0.92);
        actions.push(TimingAdjustmentAction::LoudnessNormalized);
    }

    let target_samples = seconds_to_samples(original_duration_seconds, audio.sample_rate);
    let accept = options.timing_alignment.accept_duration_diff_percent;
    let maximum_stretch = options.timing_alignment.max_stretch_diff_percent;
    let stretch_required = duration_difference_percent > accept || samples.len() > target_samples;
    let mut stretch_ratio = None;

    if stretch_required {
        if duration_difference_percent > maximum_stretch {
            critical = true;
            failure_reason = Some(format!(
                "diferença de duração {:.1}% acima do limite configurado de {:.1}%",
                duration_difference_percent, maximum_stretch
            ));
            statuses.push(TimingChunkStatus::OutOfLimit);
            statuses.push(TimingChunkStatus::NeedsManualReview);
            actions.push(TimingAdjustmentAction::ManualReviewRequired);
        } else {
            statuses.push(TimingChunkStatus::TimeStretched);
        }

        let source_len = samples.len().max(1);
        samples = time_stretch_preserving_pitch(&samples, target_samples, audio.sample_rate);
        stretch_ratio = Some(target_samples as f32 / source_len as f32);
        actions.push(TimingAdjustmentAction::TimeStretched);
    }

    if samples.len() < target_samples {
        samples.resize(target_samples, 0.0);
        actions.push(TimingAdjustmentAction::TailPreserved);
    }
    if samples.len() > target_samples {
        critical = true;
        statuses.push(TimingChunkStatus::OverlapRisk);
        failure_reason.get_or_insert_with(|| {
            "áudio gerado continuou maior que a janela original após ajuste".to_string()
        });
        samples.truncate(target_samples);
    }

    let abrupt_ending_detected = detect_abrupt_ending(
        &samples,
        audio.sample_rate,
        options.timing_alignment.min_tail_ms,
    );
    if abrupt_ending_detected {
        statuses.push(TimingChunkStatus::AbruptEndingDetected);
    }
    apply_short_fades(
        &mut samples,
        audio.sample_rate,
        options.timing_alignment.fade_out_ms,
    );
    if options.timing_alignment.fade_out_ms > 0 {
        actions.push(TimingAdjustmentAction::FadeApplied);
    }
    if statuses.is_empty() {
        statuses.push(TimingChunkStatus::Ok);
        actions.push(TimingAdjustmentAction::Accepted);
    }

    Ok(FittedSegmentAudio {
        audio: DecodedAudio::new(samples, audio.sample_rate),
        generated_duration_seconds,
        duration_difference_percent,
        statuses,
        actions,
        stretch_ratio,
        failure_reason,
        critical,
        abrupt_ending_detected,
    })
}

#[cfg(feature = "ml")]
fn compose_timeline_audio(
    placed_segments: Vec<PlacedTimelineSegment>,
    total_duration_seconds: f32,
    timing: AudioTimingProfile,
    level_settings: AudioLevelSettings,
    options: &DubbingOptions,
) -> AppResult<DecodedAudio> {
    let Some(first) = placed_segments.first() else {
        return Err(AppError::SpeechEngineUnavailable(
            "OmniVoice não gerou segmentos temporais.".to_string(),
        ));
    };
    let sample_rate = first.audio.sample_rate;
    if sample_rate == 0 {
        return Err(AppError::SpeechEngineUnavailable(
            "segmento temporal sem taxa de amostragem".to_string(),
        ));
    }

    let mut samples = vec![0.0; seconds_to_samples(total_duration_seconds, sample_rate)];
    for segment in placed_segments {
        if segment.audio.sample_rate != sample_rate {
            return Err(AppError::SpeechEngineUnavailable(format!(
                "OmniVoice retornou taxas de amostragem inconsistentes: {} e {}",
                sample_rate, segment.audio.sample_rate
            )));
        }
        let start = seconds_to_samples(segment.start_seconds, sample_rate);
        mix_segment_into_timeline(
            &mut samples,
            start,
            segment.audio.samples,
            ms_to_samples(options.timing_alignment.crossfade_ms, sample_rate),
        );
    }

    if level_settings.match_source_loudness {
        match_source_loudness(
            &mut samples,
            timing.rms_amplitude,
            timing.peak_amplitude * 0.98,
            level_settings.loudness_match_strength,
        );
    } else {
        normalize_peak(&mut samples, timing.peak_amplitude * 0.96);
    }
    apply_output_gain(&mut samples, level_settings.output_gain_db, 0.98);
    Ok(DecodedAudio::new(samples, sample_rate))
}

#[cfg(feature = "ml")]
fn mix_segment_into_timeline(
    timeline: &mut Vec<f32>,
    start: usize,
    mut segment: Vec<f32>,
    crossfade_samples: usize,
) {
    if segment.is_empty() {
        return;
    }
    let required = start.saturating_add(segment.len());
    if required > timeline.len() {
        timeline.resize(required, 0.0);
    }

    let fade = crossfade_samples.min(segment.len() / 4);
    if fade > 0 {
        for index in 0..fade {
            let scale = (index + 1) as f32 / (fade + 1) as f32;
            segment[index] = sanitize_sample(segment[index] * scale);
            let tail_index = segment.len() - 1 - index;
            segment[tail_index] = sanitize_sample(segment[tail_index] * scale);
        }
    }

    for (offset, sample) in segment.into_iter().enumerate() {
        let target = start + offset;
        timeline[target] = sanitize_sample(timeline[target] + sample);
    }
}

#[cfg(feature = "ml")]
fn duration_difference_percent(generated: f32, original: f32) -> f32 {
    if original <= f32::EPSILON || !generated.is_finite() || !original.is_finite() {
        return 100.0;
    }
    ((generated / original) - 1.0).abs() * 100.0
}

#[cfg(feature = "ml")]
fn seconds_to_samples(seconds: f32, sample_rate: u32) -> usize {
    (seconds.max(0.0) * sample_rate as f32).round().max(1.0) as usize
}

#[cfg(feature = "ml")]
fn adapt_text_for_timing(
    text: &str,
    generated_duration_seconds: f32,
    segment: &SynthesisSegmentPlan,
    attempt: u32,
) -> String {
    let target_duration_seconds = segment.duration_seconds.unwrap_or_default();
    if generated_duration_seconds > target_duration_seconds {
        shorten_timing_text(text, attempt)
    } else {
        lengthen_timing_text(text, attempt)
    }
}

#[cfg(feature = "ml")]
fn shorten_timing_text(text: &str, attempt: u32) -> String {
    let mut shortened = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for (needle, replacement) in [
        (" por favor", ""),
        (" neste momento", " agora"),
        (" exatamente", ""),
        (" realmente", ""),
        (" simplesmente", ""),
        (" absolutamente", ""),
    ] {
        shortened = shortened.replace(needle, replacement);
    }
    if attempt >= 2 {
        shortened = shortened
            .split(',')
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(shortened.trim())
            .to_string();
    }
    naturalized_terminal_punctuation(&shortened)
}

#[cfg(feature = "ml")]
fn lengthen_timing_text(text: &str, attempt: u32) -> String {
    let trimmed = text.trim().trim_end_matches(['.', '!', '?', '…']).trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if attempt >= 2 {
        format!("{trimmed}...")
    } else {
        format!("{trimmed}.")
    }
}

#[cfg(feature = "ml")]
fn time_stretch_preserving_pitch(samples: &[f32], target_len: usize, sample_rate: u32) -> Vec<f32> {
    if samples.is_empty() || target_len == 0 {
        return Vec::new();
    }
    if samples.len().abs_diff(target_len) <= 1 || sample_rate == 0 {
        return resize_with_padding(samples, target_len);
    }
    if samples.len() < ms_to_samples(120, sample_rate) {
        return resample_for_duration(samples, target_len);
    }

    let window_len =
        ms_to_samples(40, sample_rate).clamp(128, samples.len().saturating_div(2).max(128));
    let overlap = window_len / 2;
    let synthesis_hop = (window_len - overlap).max(1);
    let stretch = target_len as f32 / samples.len() as f32;
    let analysis_hop = ((synthesis_hop as f32 / stretch).round() as usize).max(1);
    let search = ms_to_samples(12, sample_rate);
    let mut output = vec![0.0; target_len.saturating_add(window_len)];
    let mut weights = vec![0.0; output.len()];
    let mut source_pos = 0usize;
    let mut target_pos = 0usize;

    while target_pos < target_len && source_pos < samples.len() {
        let candidate = if target_pos == 0 {
            source_pos
        } else {
            best_overlap_position(samples, &output, source_pos, target_pos, overlap, search)
        };
        overlap_add_window(
            samples,
            candidate,
            &mut output,
            &mut weights,
            target_pos,
            window_len,
        );
        source_pos = source_pos.saturating_add(analysis_hop);
        target_pos = target_pos.saturating_add(synthesis_hop);
    }

    for (sample, weight) in output.iter_mut().zip(weights.iter()) {
        if *weight > f32::EPSILON {
            *sample = sanitize_sample(*sample / *weight);
        }
    }
    output.truncate(target_len);
    resize_with_padding(&output, target_len)
}

#[cfg(feature = "ml")]
fn best_overlap_position(
    samples: &[f32],
    output: &[f32],
    predicted: usize,
    target_pos: usize,
    overlap: usize,
    search: usize,
) -> usize {
    let start = predicted.saturating_sub(search);
    let end = predicted
        .saturating_add(search)
        .min(samples.len().saturating_sub(overlap + 1));
    let mut best = predicted.min(end);
    let mut best_score = f32::MIN;
    for candidate in start..=end {
        let score = overlap_score(samples, output, candidate, target_pos, overlap);
        if score > best_score {
            best_score = score;
            best = candidate;
        }
    }
    best
}

#[cfg(feature = "ml")]
fn overlap_score(
    samples: &[f32],
    output: &[f32],
    source_pos: usize,
    target_pos: usize,
    overlap: usize,
) -> f32 {
    let mut score = 0.0;
    for index in 0..overlap {
        let source = samples.get(source_pos + index).copied().unwrap_or_default();
        let target = output.get(target_pos + index).copied().unwrap_or_default();
        score += source * target;
    }
    score
}

#[cfg(feature = "ml")]
fn overlap_add_window(
    samples: &[f32],
    source_pos: usize,
    output: &mut [f32],
    weights: &mut [f32],
    target_pos: usize,
    window_len: usize,
) {
    for index in 0..window_len {
        let Some(sample) = samples.get(source_pos + index).copied() else {
            break;
        };
        let Some(target) = output.get_mut(target_pos + index) else {
            break;
        };
        let weight = hann_weight(index, window_len);
        *target += sample * weight;
        if let Some(total_weight) = weights.get_mut(target_pos + index) {
            *total_weight += weight;
        }
    }
}

#[cfg(feature = "ml")]
fn hann_weight(index: usize, len: usize) -> f32 {
    if len <= 1 {
        return 1.0;
    }
    0.5 - 0.5 * ((std::f32::consts::TAU * index as f32) / (len - 1) as f32).cos()
}

#[cfg(feature = "ml")]
fn resize_with_padding(samples: &[f32], target_len: usize) -> Vec<f32> {
    let mut output = samples
        .iter()
        .copied()
        .map(sanitize_sample)
        .collect::<Vec<_>>();
    output.resize(target_len, 0.0);
    output.truncate(target_len);
    output
}

#[cfg(feature = "ml")]
fn resample_for_duration(samples: &[f32], target_len: usize) -> Vec<f32> {
    if samples.is_empty() || target_len == 0 {
        return Vec::new();
    }
    if target_len == 1 {
        return vec![samples[0]];
    }
    let scale = (samples.len() - 1) as f32 / (target_len - 1) as f32;
    (0..target_len)
        .map(|index| {
            let position = index as f32 * scale;
            let left = position.floor() as usize;
            let right = (left + 1).min(samples.len() - 1);
            let fraction = position - left as f32;
            sanitize_sample(samples[left] + (samples[right] - samples[left]) * fraction)
        })
        .collect()
}

#[cfg(feature = "ml")]
fn detect_abrupt_ending(samples: &[f32], sample_rate: u32, min_tail_ms: u32) -> bool {
    if samples.is_empty() || sample_rate == 0 {
        return false;
    }
    let tail_samples = ms_to_samples(min_tail_ms.max(50), sample_rate).min(samples.len());
    let tail = &samples[samples.len() - tail_samples..];
    let tail_rms = rms_level(tail);
    let last_peak = tail
        .iter()
        .rev()
        .take(ms_to_samples(20, sample_rate).max(1))
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    tail_rms > 0.035 && last_peak > 0.08
}

#[cfg(feature = "ml")]
fn apply_short_fades(samples: &mut [f32], sample_rate: u32, fade_ms: u32) {
    let fade_samples = ms_to_samples(fade_ms, sample_rate).min(samples.len() / 3);
    if fade_samples == 0 {
        return;
    }
    for index in 0..fade_samples {
        let fade_in = (index + 1) as f32 / (fade_samples + 1) as f32;
        samples[index] = sanitize_sample(samples[index] * fade_in);
        let tail_index = samples.len() - 1 - index;
        samples[tail_index] = sanitize_sample(samples[tail_index] * fade_in);
    }
}

#[cfg(feature = "ml")]
fn push_unique_status(statuses: &mut Vec<TimingChunkStatus>, status: TimingChunkStatus) {
    if !statuses.contains(&status) {
        statuses.push(status);
    }
}

#[cfg(feature = "ml")]
fn push_unique_action(actions: &mut Vec<TimingAdjustmentAction>, action: TimingAdjustmentAction) {
    if !actions.contains(&action) {
        actions.push(action);
    }
}

#[cfg(feature = "ml")]
fn timing_alignment_warnings(
    total_chunks: usize,
    configured_chunk_limit: u32,
    policy: ChunkLimitPolicy,
) -> Vec<String> {
    if total_chunks <= configured_chunk_limit.max(1) as usize {
        return Vec::new();
    }
    vec![format!(
        "Este áudio precisa de {total_chunks} chunks, acima do limite configurado de {configured_chunk_limit}; política aplicada: {policy:?}."
    )]
}

#[cfg(feature = "ml")]
fn chunk_report_is_critical(report: &TimingAlignmentChunkReport) -> bool {
    report.statuses.iter().any(|status| {
        matches!(
            status,
            TimingChunkStatus::OutOfLimit
                | TimingChunkStatus::NeedsManualReview
                | TimingChunkStatus::OverlapRisk
                | TimingChunkStatus::TtsFailed
        )
    })
}

#[cfg(feature = "ml")]
fn request_audio_id(request: &OwnedSynthesisRequest) -> String {
    request.source_audio.to_string_lossy().to_string()
}

#[cfg(feature = "ml")]
fn request_file_name(request: &OwnedSynthesisRequest) -> String {
    request
        .source_audio
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("audio")
        .to_string()
}

#[cfg(feature = "ml")]
fn request_safe_stem(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
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
    request.generation_config.preserve_sentence_boundaries = options.preserve_sentence_boundaries;
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
    reference_text: &str,
) -> AppResult<ShortReferencePrompt> {
    let reference = short_reference_waveform(reference_audio).map_err(|error| {
        AppError::SpeechEngineUnavailable(format!(
            "falha ao preparar referência curta para OmniVoice: {error}"
        ))
    })?;
    let text = reference_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    Ok(ShortReferencePrompt {
        audio: ReferenceAudioInput::Waveform(WaveformInput::mono(
            reference.samples,
            reference.sample_rate,
        )),
        text,
    })
}

#[cfg(feature = "ml")]
#[allow(dead_code)]
fn concatenate_segments(segments: Vec<DecodedAudio>) -> AppResult<DecodedAudio> {
    let mut segments = segments.into_iter();
    let Some(first) = segments.next() else {
        return Err(AppError::SpeechEngineUnavailable(
            "OmniVoice não gerou segmentos de áudio.".to_string(),
        ));
    };

    let sample_rate = first.sample_rate;
    let mut samples = first.samples;
    for segment in segments {
        if segment.sample_rate != sample_rate {
            return Err(AppError::SpeechEngineUnavailable(format!(
                "OmniVoice retornou taxas de amostragem inconsistentes: {} e {}",
                sample_rate, segment.sample_rate
            )));
        }
        append_segment_with_crossfade(&mut samples, segment.samples, sample_rate);
    }

    Ok(DecodedAudio::new(samples, sample_rate))
}

#[cfg(feature = "ml")]
#[allow(dead_code)]
fn append_segment_with_crossfade(samples: &mut Vec<f32>, mut next: Vec<f32>, sample_rate: u32) {
    if samples.is_empty() || next.is_empty() || sample_rate == 0 {
        samples.extend(next.into_iter().map(sanitize_sample));
        return;
    }

    let fade_samples = ms_to_samples(SEGMENT_CROSSFADE_MS, sample_rate)
        .min(samples.len() / 4)
        .min(next.len() / 4);
    if fade_samples == 0 {
        samples.extend(next.into_iter().map(sanitize_sample));
        return;
    }

    let start = samples.len() - fade_samples;
    for index in 0..fade_samples {
        let fade_in = (index + 1) as f32 / (fade_samples + 1) as f32;
        let fade_out = 1.0 - fade_in;
        samples[start + index] =
            sanitize_sample(samples[start + index] * fade_out + next[index] * fade_in);
    }
    samples.extend(next.drain(fade_samples..).map(sanitize_sample));
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
#[allow(dead_code)]
fn apply_original_timing(
    audio: DecodedAudio,
    timing: AudioTimingProfile,
    level_settings: AudioLevelSettings,
) -> DecodedAudio {
    let sample_rate = audio.sample_rate;
    let mut voice_samples = audio.samples;
    remove_dc_offset(&mut voice_samples);
    let mut voice_samples = trim_leading_silence(voice_samples);
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
fn remove_dc_offset(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }

    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    if !mean.is_finite() || mean.abs() <= f32::EPSILON {
        return;
    }

    for sample in samples {
        *sample = sanitize_sample(*sample - mean);
    }
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
    use dublagem_domain::OMNIVOICE_MAX_SYNTHESIS_SECONDS;

    fn synthesis_segments_for_test(
        request: &OwnedSynthesisRequest,
        duration_seconds: f32,
    ) -> AppResult<Vec<SynthesisSegmentPlan>> {
        synthesis_segments_for_windows(
            request,
            vec![SpeechWindow {
                start_seconds: 0.0,
                end_seconds: duration_seconds,
            }],
        )
    }

    #[test]
    fn whole_file_synthesis_uses_single_omnivoice_segment() {
        let request = OwnedSynthesisRequest {
            text: "Primeira frase. Segunda frase longa para simular um arquivo maior. Terceira frase para garantir que o texto permaneça em uma única síntese dentro do limite oficial. Quarta frase para preservar pontuação natural."
                .to_string(),
            source_text: "Original first sentence. Original second sentence.".to_string(),
            source_audio: PathBuf::from("source.wav"),
            reference_audio: PathBuf::from("source.wav"),
            reference_text: "Original first sentence. Original second sentence.".to_string(),
            output_path: PathBuf::from("out.wav"),
            options: DubbingOptions::default(),
            pinned_tags: Vec::new(),
            line_overrides: Vec::new(),
            hooks: SynthesisHooks::default(),
        };

        let segments = synthesis_segments_for_test(&request, 30.0).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].duration_seconds, Some(30.0));
        assert_eq!(segments[0].original_duration_seconds, Some(30.0));
        assert!(segments[0].text.ends_with('.'));
    }

    #[test]
    fn whole_file_synthesis_preserves_inline_native_tags() {
        let request = OwnedSynthesisRequest {
            text: "[sigh] Ola [question-ah]?".to_string(),
            source_text: "Original.".to_string(),
            source_audio: PathBuf::from("source.wav"),
            reference_audio: PathBuf::from("source.wav"),
            reference_text: "Original.".to_string(),
            output_path: PathBuf::from("out.wav"),
            options: DubbingOptions::default(),
            pinned_tags: Vec::new(),
            line_overrides: Vec::new(),
            hooks: SynthesisHooks::default(),
        };

        let segments = synthesis_segments_for_test(&request, 2.0).unwrap();

        assert_eq!(segments[0].text, "[sigh] Ola [question-ah]?");
    }

    #[test]
    fn pinned_tags_are_applied_to_whole_file_synthesis() {
        let request = OwnedSynthesisRequest {
            text: "Ola mundo".to_string(),
            source_text: "Original.".to_string(),
            source_audio: PathBuf::from("source.wav"),
            reference_audio: PathBuf::from("source.wav"),
            reference_text: "Original.".to_string(),
            output_path: PathBuf::from("out.wav"),
            options: DubbingOptions::default(),
            pinned_tags: vec!["[sigh]".to_string()],
            line_overrides: Vec::new(),
            hooks: SynthesisHooks::default(),
        };

        let segments = synthesis_segments_for_test(&request, 2.0).unwrap();

        assert_eq!(segments[0].text, "[sigh] Ola mundo.");
    }

    #[test]
    fn whole_file_synthesis_accepts_long_form_duration_for_chunked_inference() {
        let request = OwnedSynthesisRequest {
            text: "Texto acima do limite.".to_string(),
            source_text: "Original.".to_string(),
            source_audio: PathBuf::from("source.wav"),
            reference_audio: PathBuf::from("source.wav"),
            reference_text: "Original.".to_string(),
            output_path: PathBuf::from("out.wav"),
            options: DubbingOptions::default(),
            pinned_tags: Vec::new(),
            line_overrides: Vec::new(),
            hooks: SynthesisHooks::default(),
        };

        let result = synthesis_segments_for_test(&request, OMNIVOICE_MAX_SYNTHESIS_SECONDS + 0.01);

        let segments = result.expect("long form segment");
        assert_eq!(segments.len(), 1);
        assert_eq!(
            segments[0].duration_seconds,
            Some(OMNIVOICE_MAX_SYNTHESIS_SECONDS + 0.01)
        );
    }

    #[test]
    fn whole_file_synthesis_merges_excess_acoustic_windows_for_single_text_unit() {
        let request = OwnedSynthesisRequest {
            text: "Fique em guarda enquanto a névoa baixa".to_string(),
            source_text: "Stay alert while the fog drops.".to_string(),
            source_audio: PathBuf::from("source.wav"),
            reference_audio: PathBuf::from("source.wav"),
            reference_text: "Stay alert while the fog drops.".to_string(),
            output_path: PathBuf::from("out.wav"),
            options: DubbingOptions::default(),
            pinned_tags: Vec::new(),
            line_overrides: Vec::new(),
            hooks: SynthesisHooks::default(),
        };

        let segments = synthesis_segments_for_windows(
            &request,
            vec![
                SpeechWindow {
                    start_seconds: 0.0,
                    end_seconds: 2.0,
                },
                SpeechWindow {
                    start_seconds: 2.2,
                    end_seconds: 4.0,
                },
                SpeechWindow {
                    start_seconds: 4.4,
                    end_seconds: 6.0,
                },
            ],
        )
        .unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_seconds, 0.0);
        assert_eq!(segments[0].end_seconds, 6.0);
        assert_eq!(segments[0].text, "Fique em guarda enquanto a névoa baixa.");
    }

    #[test]
    fn whole_file_synthesis_prefers_comma_boundary_without_repeated_words() {
        let request = OwnedSynthesisRequest {
            text: "Uno com a natureza, mas nunca vi uma meditação tão poderosa antes.".to_string(),
            source_text: "One with nature, but I have never seen meditation this powerful before."
                .to_string(),
            source_audio: PathBuf::from("source.wav"),
            reference_audio: PathBuf::from("source.wav"),
            reference_text: "One with nature.".to_string(),
            output_path: PathBuf::from("out.wav"),
            options: DubbingOptions::default(),
            pinned_tags: Vec::new(),
            line_overrides: Vec::new(),
            hooks: SynthesisHooks::default(),
        };

        let segments = synthesis_segments_for_windows(
            &request,
            vec![
                SpeechWindow {
                    start_seconds: 0.0,
                    end_seconds: 2.0,
                },
                SpeechWindow {
                    start_seconds: 2.3,
                    end_seconds: 4.0,
                },
                SpeechWindow {
                    start_seconds: 4.4,
                    end_seconds: 6.0,
                },
            ],
        )
        .unwrap();

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "Uno com a natureza.");
        assert_eq!(
            segments[1].text,
            "mas nunca vi uma meditação tão poderosa antes."
        );
        assert!(!segments[0].text.contains("nunca"));
    }

    #[test]
    fn line_overrides_become_independent_segments_with_line_settings() {
        let base_settings = NativeSynthesisSettings::default();
        let line_settings = NativeSynthesisSettings {
            speed: Some(1.2),
            output_gain_db: -6.0,
            ..base_settings.clone()
        };
        let request = OwnedSynthesisRequest {
            text: "Primeira frase. Segunda frase.".to_string(),
            source_text: "Original first sentence. Original second sentence.".to_string(),
            source_audio: PathBuf::from("source.wav"),
            reference_audio: PathBuf::from("source.wav"),
            reference_text: "Original first sentence. Original second sentence.".to_string(),
            output_path: PathBuf::from("out.wav"),
            options: DubbingOptions {
                native_synthesis: base_settings.clone(),
                ..DubbingOptions::default()
            },
            pinned_tags: Vec::new(),
            line_overrides: vec![
                LineSynthesisOverride {
                    line_index: 0,
                    target_text: "[sigh] Primeira frase.".to_string(),
                    tags: vec!["[sigh]".to_string()],
                    settings: line_settings,
                },
                LineSynthesisOverride {
                    line_index: 1,
                    target_text: "Segunda frase.".to_string(),
                    tags: Vec::new(),
                    settings: base_settings,
                },
            ],
            hooks: SynthesisHooks::default(),
        };

        let segments = synthesis_segments_for_windows(
            &request,
            vec![
                SpeechWindow {
                    start_seconds: 0.0,
                    end_seconds: 6.0,
                },
                SpeechWindow {
                    start_seconds: 7.0,
                    end_seconds: 12.0,
                },
            ],
        )
        .unwrap();

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].duration_seconds, Some(6.0));
        assert_eq!(segments[0].text, "[sigh] Primeira frase.");
        assert_eq!(segments[0].settings.speed, Some(1.2));
        assert_eq!(segments[1].settings, request.options.native_synthesis);
    }

    #[test]
    fn generation_request_maps_design_settings_without_clone_prompt() {
        let options = DubbingOptions {
            preserve_sentence_boundaries: true,
            ..DubbingOptions::default()
        };
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
        assert!(request.generation_config.preserve_sentence_boundaries);
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
    fn generated_segment_cleanup_preserves_trailing_tail() {
        let samples = vec![0.0, 0.0, 0.5, 0.25, 0.001, 0.0];

        let cleaned = trim_leading_silence(samples);

        assert_eq!(cleaned, vec![0.5, 0.25, 0.001, 0.0]);
    }

    #[test]
    fn sync_cleanup_removes_dc_offset_before_trimming() {
        let mut samples = vec![0.2, 0.4, 0.6, 0.8];

        remove_dc_offset(&mut samples);
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;

        assert!(mean.abs() <= f32::EPSILON);
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
        assert_eq!(
            effective_synthesis_tags(
                &["[sigh]".to_string()],
                &["[sigh]".to_string(), "[surprise-oh]".to_string()]
            ),
            vec!["[sigh]".to_string(), "[surprise-oh]".to_string()]
        );
    }

    #[test]
    fn sanitized_fallback_line_overrides_strip_native_tags() {
        let base_settings = NativeSynthesisSettings::default();
        let request = OwnedSynthesisRequest {
            text: "Fallback sem marcador.".to_string(),
            source_text: "Original.".to_string(),
            source_audio: PathBuf::from("source.wav"),
            reference_audio: PathBuf::from("source.wav"),
            reference_text: "Original.".to_string(),
            output_path: PathBuf::from("out.wav"),
            options: DubbingOptions::default(),
            pinned_tags: Vec::new(),
            line_overrides: vec![LineSynthesisOverride {
                line_index: 0,
                target_text: dublagem_domain::strip_omnivoice_native_tags(
                    "Ola [surprise-oh] mundo.",
                ),
                tags: Vec::new(),
                settings: base_settings,
            }],
            hooks: SynthesisHooks::default(),
        };

        let segments = synthesis_segments_for_test(&request, 2.0).unwrap();

        assert_eq!(segments[0].text, "Ola mundo.");
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
        let audio = DecodedAudio::new(
            (0..100)
                .map(|index| if index % 2 == 0 { 0.04 } else { -0.04 })
                .collect(),
            1_000,
        );

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
