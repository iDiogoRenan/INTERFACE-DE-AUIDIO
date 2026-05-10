use crate::error::{AppError, AppResult};
use dublagem_domain::{
    AudioFileEntry, AudioFileStatus, AudioMetadata, CachedTranscription, QualityClassification,
    QualityReport,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};
#[cfg(feature = "ml")]
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    errors::Error as SymphoniaError,
};
use symphonia::core::{
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

pub const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "wem", "ogg", "flac"];
const TRANSCRIPTION_CACHE_FILE: &str = "transcricoes_cache.json";
const FAMILY_MARKER_TOKENS: &[&str] = &["questdialog", "narration", "player"];
#[cfg(feature = "ml")]
const WHISPER_SAMPLE_RATE: u32 = 16_000;
#[cfg(feature = "ml")]
pub const TTS_SAMPLE_RATE: u32 = 24_000;
#[cfg(feature = "ml")]
pub const SHORT_REFERENCE_TARGET_SECONDS: f32 = 8.0;
#[cfg(feature = "ml")]
pub const SHORT_REFERENCE_MIN_SECONDS: f32 = 3.0;
#[cfg(feature = "ml")]
pub const SHORT_REFERENCE_MAX_SECONDS: f32 = 10.0;

#[cfg(feature = "ml")]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

#[cfg(feature = "ml")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioTimingProfile {
    pub total_ms: u32,
    pub leading_silence_ms: u32,
    pub trailing_silence_ms: u32,
    pub voice_ms: u32,
    pub peak_amplitude: f32,
    pub rms_amplitude: f32,
}

#[cfg(feature = "ml")]
impl AudioTimingProfile {
    pub fn target_voice_duration_seconds(self) -> Option<f32> {
        (self.voice_ms > 0).then_some(self.voice_ms as f32 / 1000.0)
    }
}

#[cfg(feature = "ml")]
#[derive(Debug, Clone)]
pub struct ShortReferenceWaveform {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_seconds: f32,
    pub source_duration_seconds: f32,
    pub start_seconds: f32,
}

pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
        .unwrap_or(false)
}

pub fn scan_audio_folder(
    input_dir: &Path,
    output_dir: Option<&Path>,
) -> AppResult<Vec<AudioFileEntry>> {
    if !input_dir.is_dir() {
        return Err(AppError::InvalidPath(input_dir.to_path_buf()));
    }

    let mut entries = Vec::new();
    let transcription_cache = load_transcription_cache(output_dir);
    for entry in std::fs::read_dir(input_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !is_audio_file(&path) {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let family = audio_family_from_filename(&name);
        let status = status_for_file(&name, output_dir);
        let transcription = transcription_cache.get(&name).cloned();
        entries.push(AudioFileEntry {
            family,
            metadata: get_audio_metadata(&path).ok(),
            name,
            path,
            status,
            transcription,
        });
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

pub fn save_transcription_cache(
    output_dir: &Path,
    file_name: &str,
    source_text: &str,
    target_text: &str,
) -> AppResult<()> {
    std::fs::create_dir_all(output_dir)?;
    let path = output_dir.join(TRANSCRIPTION_CACHE_FILE);
    let mut cache = if path.exists() {
        read_transcription_cache_file(&path)?
    } else {
        BTreeMap::new()
    };
    cache.insert(
        file_name.to_string(),
        TranscriptionCacheEntry::from_texts(source_text, target_text),
    );
    let payload = serde_json::to_string_pretty(&cache)?;
    std::fs::write(path, payload)?;
    Ok(())
}

pub fn dubbed_output_path(output_dir: &Path, file_name: &str) -> PathBuf {
    output_dir
        .join(audio_family_from_filename(file_name))
        .join(file_name)
}

pub fn get_audio_metadata(path: &Path) -> AppResult<AudioMetadata> {
    let extension = extension(path);
    if extension == "wem" {
        return Err(AppError::UnsupportedCodec(
            "Wwise WEM precisa de decodificador Rust validado antes de entrar no fluxo".to_string(),
        ));
    }

    let file = File::open(path)?;
    let media_source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if !extension.is_empty() {
        hint.with_extension(&extension);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            media_source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| AppError::UnsupportedCodec(error.to_string()))?;
    let format = probed.format;
    let track = format.default_track().ok_or_else(|| {
        AppError::UnsupportedCodec("arquivo sem faixa de áudio padrão".to_string())
    })?;
    let params = &track.codec_params;
    let duration_seconds = match (params.n_frames, params.sample_rate) {
        (Some(frames), Some(sample_rate)) if sample_rate > 0 => {
            Some(frames as f64 / f64::from(sample_rate))
        }
        _ => None,
    };

    Ok(AudioMetadata {
        duration_seconds,
        sample_rate: params.sample_rate,
        channels: params.channels.map(|channels| channels.count() as u16),
        format: extension,
    })
}

pub fn audio_family_from_filename(filename: &str) -> String {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(filename)
        .trim_matches('_')
        .to_lowercase();
    let tokens = stem
        .split('_')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        return "outros".to_string();
    }

    let marker_index = tokens
        .iter()
        .position(|token| FAMILY_MARKER_TOKENS.contains(token));
    let sequence_index = tokens
        .iter()
        .position(|token| is_sequence_token(token))
        .unwrap_or(tokens.len());
    let take_until = marker_index.unwrap_or(sequence_index);
    let mut family_tokens = tokens.into_iter().take(take_until).collect::<Vec<_>>();

    while family_tokens
        .last()
        .map(|token| is_sequence_token(token))
        .unwrap_or(false)
    {
        family_tokens.pop();
    }

    if family_tokens.is_empty() {
        "outros".to_string()
    } else {
        family_tokens.join("_")
    }
}

pub fn read_wav_mono_f32(path: &Path) -> AppResult<Vec<f32>> {
    let metadata = get_audio_metadata(path)?;
    if metadata.format != "wav" {
        return Err(AppError::UnsupportedCodec(format!(
            "analise de amostras implementada apenas para wav, recebido {}",
            metadata.format
        )));
    }
    let decoded = read_wav_interleaved_f32(path)?;
    Ok(mix_interleaved_to_mono(&decoded.samples, decoded.channels))
}

#[cfg(feature = "ml")]
pub fn read_audio_mono_16khz_f32(path: &Path) -> AppResult<Vec<f32>> {
    let decoded = decode_audio_mono_f32(path)?;
    if decoded.sample_rate == WHISPER_SAMPLE_RATE {
        return Ok(decoded.samples);
    }
    Ok(resample_linear_mono(
        &decoded.samples,
        decoded.sample_rate,
        WHISPER_SAMPLE_RATE,
    ))
}

#[cfg(feature = "ml")]
pub fn short_reference_waveform(path: &Path) -> AppResult<ShortReferenceWaveform> {
    let decoded = decode_audio_mono_f32(path)?;
    let samples = if decoded.sample_rate == TTS_SAMPLE_RATE {
        decoded.samples
    } else {
        resample_linear_mono(&decoded.samples, decoded.sample_rate, TTS_SAMPLE_RATE)
    }
    .into_iter()
    .map(sanitize_wav_sample)
    .collect::<Vec<_>>();

    if samples.is_empty() {
        return Err(AppError::UnsupportedCodec(
            "áudio de referência vazio".to_string(),
        ));
    }

    let target_samples = seconds_to_sample_count(SHORT_REFERENCE_TARGET_SECONDS, TTS_SAMPLE_RATE);
    let min_samples = seconds_to_sample_count(SHORT_REFERENCE_MIN_SECONDS, TTS_SAMPLE_RATE);
    let max_samples = seconds_to_sample_count(SHORT_REFERENCE_MAX_SECONDS, TTS_SAMPLE_RATE);
    let blocks = active_sample_blocks(&samples, TTS_SAMPLE_RATE, 40.0);
    let start = blocks.first().map(|range| range.start).unwrap_or(0);
    let end = start.saturating_add(target_samples).min(samples.len());
    let mut reference = samples[start..end].to_vec();

    if reference.len() < min_samples && !blocks.is_empty() {
        reference = concatenate_active_blocks(&samples, &blocks, target_samples);
    }
    if reference.is_empty() {
        reference = samples.iter().copied().take(target_samples).collect();
    }
    if reference.len() > max_samples {
        reference.truncate(max_samples);
    }
    if reference.is_empty() {
        return Err(AppError::UnsupportedCodec(
            "não foi possível extrair referência curta válida".to_string(),
        ));
    }

    Ok(ShortReferenceWaveform {
        duration_seconds: reference.len() as f32 / TTS_SAMPLE_RATE as f32,
        source_duration_seconds: samples.len() as f32 / TTS_SAMPLE_RATE as f32,
        start_seconds: start as f32 / TTS_SAMPLE_RATE as f32,
        samples: reference,
        sample_rate: TTS_SAMPLE_RATE,
    })
}

#[cfg(feature = "ml")]
pub fn write_short_reference_wav(
    source_path: &Path,
    output_path: &Path,
) -> AppResult<ShortReferenceWaveform> {
    let reference = short_reference_waveform(source_path)?;
    if let Some(parent) = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    write_pcm16_wav_mono(output_path, reference.sample_rate, &reference.samples)?;
    Ok(reference)
}

#[cfg(feature = "ml")]
pub fn decode_audio_mono_f32(path: &Path) -> AppResult<DecodedAudio> {
    let extension = extension(path);
    if extension == "wem" {
        return Err(AppError::UnsupportedCodec(
            "Wwise WEM precisa de decodificador Rust validado antes de entrar no fluxo".to_string(),
        ));
    }

    let file = File::open(path)?;
    let media_source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if !extension.is_empty() {
        hint.with_extension(&extension);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            media_source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| AppError::UnsupportedCodec(error.to_string()))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| {
        AppError::UnsupportedCodec("arquivo sem faixa de áudio padrão".to_string())
    })?;
    if track.codec_params.codec == CODEC_TYPE_NULL {
        return Err(AppError::UnsupportedCodec(
            "arquivo sem codec de áudio detectável".to_string(),
        ));
    }

    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|error| AppError::UnsupportedCodec(error.to_string()))?;
    let mut samples = Vec::new();
    let mut sample_rate = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(error) => return Err(AppError::UnsupportedCodec(error.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }

        let audio = match decoder.decode(&packet) {
            Ok(audio) => audio,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(AppError::UnsupportedCodec(error.to_string())),
        };
        let spec = *audio.spec();
        let channels = spec.channels.count();
        if channels == 0 || spec.rate == 0 {
            return Err(AppError::UnsupportedCodec(
                "áudio decodificado sem canais ou taxa de amostragem".to_string(),
            ));
        }
        if let Some(existing_rate) = sample_rate {
            if existing_rate != spec.rate {
                return Err(AppError::UnsupportedCodec(
                    "mudança de taxa de amostragem no meio do fluxo não suportada".to_string(),
                ));
            }
        } else {
            sample_rate = Some(spec.rate);
        }

        let mut buffer = SampleBuffer::<f32>::new(audio.capacity() as u64, spec);
        buffer.copy_interleaved_ref(audio);
        for frame in buffer.samples().chunks_exact(channels) {
            let mono = frame.iter().copied().sum::<f32>() / channels as f32;
            samples.push(mono.clamp(-1.0, 1.0));
        }
    }

    if samples.is_empty() {
        return Err(AppError::UnsupportedCodec(
            "áudio sem amostras decodificáveis".to_string(),
        ));
    }

    Ok(DecodedAudio {
        samples,
        sample_rate: sample_rate.unwrap_or(WHISPER_SAMPLE_RATE),
    })
}

#[cfg(feature = "ml")]
pub fn audio_timing_profile(path: &Path, pad_ms: u32) -> AppResult<AudioTimingProfile> {
    let decoded = decode_audio_mono_f32(path)?;
    Ok(audio_timing_profile_from_samples(
        &decoded.samples,
        decoded.sample_rate,
        pad_ms,
    ))
}

pub fn quality_report(samples: &[f32]) -> QualityReport {
    if samples.is_empty() {
        return QualityReport {
            is_acceptable: false,
            score: 0,
            classification: QualityClassification::Critica,
            summary: "Crítica: áudio vazio.".to_string(),
            zcr_average: 0.0,
            peak_amplitude: 0.0,
            rms: 0.0,
            issues: vec!["Audio vazio.".to_string()],
        };
    }

    let zero_crossings = samples
        .windows(2)
        .filter(|window| window[0].signum() != window[1].signum())
        .count();
    let zcr_average = zero_crossings as f32 / samples.len() as f32;
    let peak_amplitude = samples
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let rms =
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt();
    let mut issues = Vec::new();
    let mut score: i16 = 100;

    if peak_amplitude <= 0.0001 {
        issues.push("Audio praticamente mudo.".to_string());
        score -= 100;
    }
    if peak_amplitude > 0.985 {
        issues.push("Audio proximo de clipping.".to_string());
        score -= 28;
    }
    if zcr_average > 0.45 {
        issues.push(format!("ZCR alto demais ({zcr_average:.2})."));
        score -= 18;
    }
    if rms < 0.015 {
        issues.push("Fala muito baixa ou com silêncio excessivo.".to_string());
        score -= 18;
    }
    if rms > 0.35 {
        issues.push("Volume médio alto; pode haver compressão ou distorção.".to_string());
        score -= 10;
    }
    let score = score.clamp(0, 100) as u8;
    let classification = quality_classification(score);

    QualityReport {
        is_acceptable: score >= 55,
        score,
        classification,
        summary: quality_summary(classification, &issues),
        zcr_average,
        peak_amplitude,
        rms,
        issues,
    }
}

fn quality_classification(score: u8) -> QualityClassification {
    match score {
        90..=100 => QualityClassification::Excelente,
        75..=89 => QualityClassification::Boa,
        55..=74 => QualityClassification::Aceitavel,
        35..=54 => QualityClassification::Ruim,
        _ => QualityClassification::Critica,
    }
}

fn quality_summary(classification: QualityClassification, issues: &[String]) -> String {
    if issues.is_empty() {
        return format!(
            "{}: fala clara, pouco ruído e volume estável.",
            classification.label_pt_br()
        );
    }

    let reason = issues
        .iter()
        .take(2)
        .map(|issue| issue.trim_end_matches('.'))
        .collect::<Vec<_>>()
        .join("; ");
    format!("{}: {}.", classification.label_pt_br(), reason)
}

fn status_for_file(name: &str, output_dir: Option<&Path>) -> AudioFileStatus {
    output_dir
        .map(|dir| dubbed_output_path(dir, name).exists())
        .filter(|exists| *exists)
        .map(|_| AudioFileStatus::Dubbed)
        .unwrap_or(AudioFileStatus::Pending)
}

fn load_transcription_cache(output_dir: Option<&Path>) -> BTreeMap<String, CachedTranscription> {
    output_dir
        .map(|dir| dir.join(TRANSCRIPTION_CACHE_FILE))
        .filter(|path| path.is_file())
        .and_then(|path| read_transcription_cache_file(&path).ok())
        .map(|cache| {
            cache
                .into_iter()
                .filter_map(|(name, entry)| entry.into_cached().map(|cached| (name, cached)))
                .collect()
        })
        .unwrap_or_default()
}

fn read_transcription_cache_file(
    path: &Path,
) -> AppResult<BTreeMap<String, TranscriptionCacheEntry>> {
    let payload = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&payload)?)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptionCacheEntry {
    #[serde(default)]
    source_text: String,
    #[serde(default)]
    target_text: String,
    #[serde(default)]
    en: String,
    #[serde(default)]
    pt: String,
}

impl TranscriptionCacheEntry {
    fn from_texts(source_text: &str, target_text: &str) -> Self {
        Self {
            source_text: source_text.to_string(),
            target_text: target_text.to_string(),
            en: source_text.to_string(),
            pt: target_text.to_string(),
        }
    }

    fn into_cached(self) -> Option<CachedTranscription> {
        let source_text = first_non_empty([self.source_text, self.en])?;
        let target_text = first_non_empty([self.target_text, self.pt])?;
        Some(CachedTranscription {
            source_text,
            target_text,
        })
    }
}

fn first_non_empty(values: [String; 2]) -> Option<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn is_sequence_token(token: &str) -> bool {
    token.chars().all(|char| char.is_ascii_digit()) && (token.len() > 1 || token.starts_with('0'))
}

#[derive(Debug, Clone)]
struct WavInterleavedSamples {
    samples: Vec<f32>,
    channels: usize,
}

fn read_wav_interleaved_f32(path: &Path) -> AppResult<WavInterleavedSamples> {
    let mut reader = hound::WavReader::open(path).map_err(map_wav_error)?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels);
    if channels == 0 || spec.sample_rate == 0 {
        return Err(AppError::UnsupportedCodec(
            "wav sem canais ou taxa de amostragem válidos".to_string(),
        ));
    }

    let samples = match spec.sample_format {
        hound::SampleFormat::Float => read_float_wav_samples(&mut reader, spec.bits_per_sample)?,
        hound::SampleFormat::Int => read_integer_wav_samples(&mut reader, spec.bits_per_sample)?,
    };
    if samples.is_empty() {
        return Err(AppError::UnsupportedCodec(
            "wav sem amostras decodificáveis".to_string(),
        ));
    }

    Ok(WavInterleavedSamples { samples, channels })
}

fn read_float_wav_samples<R: std::io::Read>(
    reader: &mut hound::WavReader<R>,
    bits_per_sample: u16,
) -> AppResult<Vec<f32>> {
    if bits_per_sample != 32 {
        return Err(AppError::UnsupportedCodec(format!(
            "wav float de {bits_per_sample} bits não suportado"
        )));
    }

    reader
        .samples::<f32>()
        .map(|sample| sample.map(sanitize_wav_sample).map_err(map_wav_error))
        .collect()
}

fn read_integer_wav_samples<R: std::io::Read>(
    reader: &mut hound::WavReader<R>,
    bits_per_sample: u16,
) -> AppResult<Vec<f32>> {
    if !(2..=32).contains(&bits_per_sample) {
        return Err(AppError::UnsupportedCodec(format!(
            "wav PCM de {bits_per_sample} bits não suportado"
        )));
    }

    let scale = integer_wav_scale(bits_per_sample);
    if bits_per_sample <= 16 {
        reader
            .samples::<i16>()
            .map(|sample| {
                sample
                    .map(|value| sanitize_wav_sample(value as f32 / scale))
                    .map_err(map_wav_error)
            })
            .collect()
    } else {
        reader
            .samples::<i32>()
            .map(|sample| {
                sample
                    .map(|value| sanitize_wav_sample(value as f32 / scale))
                    .map_err(map_wav_error)
            })
            .collect()
    }
}

fn integer_wav_scale(bits_per_sample: u16) -> f32 {
    if bits_per_sample >= 32 {
        i32::MAX as f32
    } else {
        ((1_i64 << (bits_per_sample - 1)) - 1) as f32
    }
}

fn mix_interleaved_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }

    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
        .map(sanitize_wav_sample)
        .collect()
}

fn sanitize_wav_sample(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

fn map_wav_error(error: hound::Error) -> AppError {
    match error {
        hound::Error::IoError(error) => AppError::Io(error.to_string()),
        error => AppError::UnsupportedCodec(error.to_string()),
    }
}

#[cfg(feature = "ml")]
fn seconds_to_sample_count(seconds: f32, sample_rate: u32) -> usize {
    (seconds.max(0.0) * sample_rate as f32).round().max(1.0) as usize
}

#[cfg(feature = "ml")]
fn active_sample_blocks(
    samples: &[f32],
    sample_rate: u32,
    top_db: f32,
) -> Vec<std::ops::Range<usize>> {
    if samples.is_empty() {
        return Vec::new();
    }
    let peak = samples
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()));
    if peak <= f32::EPSILON {
        return Vec::new();
    }

    let threshold = (peak * 10_f32.powf(-top_db / 20.0)).max(0.0001);
    let mut raw_ranges = Vec::new();
    let mut start = None;
    for (index, sample) in samples.iter().enumerate() {
        if sample.abs() > threshold {
            start.get_or_insert(index);
        } else if let Some(range_start) = start.take() {
            raw_ranges.push(range_start..index);
        }
    }
    if let Some(range_start) = start {
        raw_ranges.push(range_start..samples.len());
    }

    merge_short_reference_ranges(raw_ranges, ms_to_samples(100, sample_rate))
}

#[cfg(feature = "ml")]
fn merge_short_reference_ranges(
    ranges: Vec<std::ops::Range<usize>>,
    maximum_gap: usize,
) -> Vec<std::ops::Range<usize>> {
    let mut merged: Vec<std::ops::Range<usize>> = Vec::new();
    for range in ranges.into_iter().filter(|range| range.start < range.end) {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end.saturating_add(maximum_gap) {
                last.end = last.end.max(range.end);
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

#[cfg(feature = "ml")]
fn concatenate_active_blocks(
    samples: &[f32],
    blocks: &[std::ops::Range<usize>],
    limit_samples: usize,
) -> Vec<f32> {
    let mut output = Vec::with_capacity(limit_samples.min(samples.len()));
    for block in blocks {
        if output.len() >= limit_samples {
            break;
        }
        let available = limit_samples - output.len();
        let end = block.start.saturating_add(available).min(block.end);
        output.extend_from_slice(&samples[block.start..end]);
    }
    output
}

#[cfg(feature = "ml")]
fn write_pcm16_wav_mono(path: &Path, sample_rate: u32, samples: &[f32]) -> AppResult<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(map_wav_error)?;
    for sample in samples {
        let value = (sanitize_wav_sample(*sample).clamp(-0.999, 0.999) * i16::MAX as f32) as i16;
        writer.write_sample(value).map_err(map_wav_error)?;
    }
    writer.finalize().map_err(map_wav_error)
}

#[cfg(feature = "ml")]
fn resample_linear_mono(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 || source_rate == target_rate {
        return samples.to_vec();
    }

    let output_len =
        ((samples.len() as u128 * target_rate as u128) / source_rate as u128).max(1) as usize;
    let ratio = source_rate as f64 / target_rate as f64;
    (0..output_len)
        .map(|index| {
            let source_position = index as f64 * ratio;
            let left = source_position.floor() as usize;
            let right = (left + 1).min(samples.len() - 1);
            let fraction = (source_position - left as f64) as f32;
            samples[left] + (samples[right] - samples[left]) * fraction
        })
        .collect()
}

#[cfg(feature = "ml")]
fn audio_timing_profile_from_samples(
    samples: &[f32],
    sample_rate: u32,
    pad_ms: u32,
) -> AudioTimingProfile {
    if samples.is_empty() || sample_rate == 0 {
        return AudioTimingProfile {
            total_ms: 0,
            leading_silence_ms: 0,
            trailing_silence_ms: 0,
            voice_ms: 0,
            peak_amplitude: 0.95,
            rms_amplitude: 0.08,
        };
    }

    let total_ms = samples_to_ms(samples.len(), sample_rate);
    let peak = samples
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()))
        .clamp(0.05, 0.98);
    let active_range = active_sample_range(samples, peak);
    let rms_amplitude = active_range
        .map(|(start, end)| rms_amplitude(&samples[start..=end]))
        .unwrap_or_else(|| rms_amplitude(samples))
        .clamp(0.01, 0.5);
    let (leading_silence_ms, raw_trailing_silence_ms) = active_range
        .map(|(start, end)| {
            (
                samples_to_ms(start, sample_rate),
                samples_to_ms(samples.len().saturating_sub(end + 1), sample_rate),
            )
        })
        .unwrap_or((0, 0));

    let guard_ms = final_guard_ms(pad_ms);
    let trailing_limit = (total_ms.saturating_mul(30) / 100).max(35);
    let trailing_silence_ms = raw_trailing_silence_ms.max(guard_ms).min(trailing_limit);
    let leading_limit = total_ms
        .saturating_sub(trailing_silence_ms)
        .saturating_sub(80);
    let leading_silence_ms = leading_silence_ms.min(leading_limit);
    let voice_ms = total_ms
        .saturating_sub(leading_silence_ms)
        .saturating_sub(trailing_silence_ms)
        .max(80);

    AudioTimingProfile {
        total_ms,
        leading_silence_ms,
        trailing_silence_ms,
        voice_ms,
        peak_amplitude: peak,
        rms_amplitude,
    }
}

#[cfg(feature = "ml")]
fn rms_amplitude(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

#[cfg(feature = "ml")]
pub fn active_sample_range(samples: &[f32], peak_amplitude: f32) -> Option<(usize, usize)> {
    let threshold = (peak_amplitude * 10_f32.powf(-35.0 / 20.0)).max(0.0001);
    let start = samples.iter().position(|sample| sample.abs() > threshold)?;
    let end = samples
        .iter()
        .rposition(|sample| sample.abs() > threshold)?;
    Some((start, end))
}

#[cfg(feature = "ml")]
pub fn final_guard_ms(pad_ms: u32) -> u32 {
    let candidate = if pad_ms > 0 { pad_ms } else { 120 };
    candidate.clamp(80, 450)
}

#[cfg(feature = "ml")]
pub fn samples_to_ms(sample_count: usize, sample_rate: u32) -> u32 {
    if sample_rate == 0 {
        return 0;
    }
    (((sample_count as u64 * 1000) + (u64::from(sample_rate) / 2)) / u64::from(sample_rate)) as u32
}

#[cfg(feature = "ml")]
pub fn ms_to_samples(duration_ms: u32, sample_rate: u32) -> usize {
    ((u64::from(duration_ms) * u64::from(sample_rate)) / 1000) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_wav_mono_f32(path: &Path, sample_rate: u32, samples: &[f32]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for sample in samples {
            writer.write_sample(*sample).expect("write sample");
        }
        writer.finalize().expect("finalize wav");
    }

    fn write_test_wav_mono_i16(path: &Path, sample_rate: u32, samples: &[i16]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for sample in samples {
            writer.write_sample(*sample).expect("write sample");
        }
        writer.finalize().expect("finalize wav");
    }

    #[test]
    fn loads_legacy_transcription_cache_for_scanned_files() {
        let input_dir = tempfile::tempdir().expect("input tempdir");
        let output_dir = tempfile::tempdir().expect("output tempdir");
        let file_name = "line_cache.wav";

        std::fs::write(input_dir.path().join(file_name), b"not real wav").expect("input audio");
        let output_path = dubbed_output_path(output_dir.path(), file_name);
        std::fs::create_dir_all(output_path.parent().expect("output parent"))
            .expect("family output dir");
        std::fs::write(output_path, b"dubbed audio").expect("output audio");
        std::fs::write(
            output_dir.path().join(TRANSCRIPTION_CACHE_FILE),
            r#"{"line_cache.wav":{"en":"Original cached text.","pt":"Texto traduzido em cache."}}"#,
        )
        .expect("legacy cache");

        let files = scan_audio_folder(input_dir.path(), Some(output_dir.path())).expect("scan");

        let file = files
            .iter()
            .find(|entry| entry.name == file_name)
            .expect("cached file entry");
        assert_eq!(file.status, AudioFileStatus::Dubbed);
        let transcription = file.transcription.as_ref().expect("cached transcription");
        assert_eq!(transcription.source_text, "Original cached text.");
        assert_eq!(transcription.target_text, "Texto traduzido em cache.");
    }

    #[test]
    fn saves_transcription_cache_in_legacy_and_frontend_shapes() {
        let output_dir = tempfile::tempdir().expect("output tempdir");

        save_transcription_cache(
            output_dir.path(),
            "line_saved.wav",
            "Fresh source text.",
            "Texto destino novo.",
        )
        .expect("save transcription cache");

        let cache_payload =
            std::fs::read_to_string(output_dir.path().join(TRANSCRIPTION_CACHE_FILE))
                .expect("cache payload");
        assert!(cache_payload.contains("\"sourceText\""));
        assert!(cache_payload.contains("\"targetText\""));
        assert!(cache_payload.contains("\"en\""));
        assert!(cache_payload.contains("\"pt\""));

        let cache = load_transcription_cache(Some(output_dir.path()));
        let transcription = cache
            .get("line_saved.wav")
            .expect("saved cached transcription");
        assert_eq!(transcription.source_text, "Fresh source text.");
        assert_eq!(transcription.target_text, "Texto destino novo.");
    }

    #[test]
    fn extracts_audio_families_from_known_game_names() {
        let cases = [
            (
                "_ancientstonegolem_9000_boss_00_00001.wav",
                "ancientstonegolem",
            ),
            (
                "dragon_common_dragon_boss_9000_narration_00005.wav",
                "dragon_common_dragon_boss",
            ),
            ("ndw_adult_1_questdialog_hello_00664.wav", "ndw_adult_1"),
            ("unique_kliff_0090_0120_player_00000.wav", "unique_kliff"),
        ];

        for (filename, expected) in cases {
            assert_eq!(audio_family_from_filename(filename), expected);
        }
    }

    #[test]
    fn builds_dubbed_output_path_inside_family_folder() {
        let path = dubbed_output_path(
            Path::new(r"E:\audio\saida"),
            "unique_kliff_0090_0120_player_00000.wav",
        );

        assert!(path
            .ends_with(Path::new("unique_kliff").join("unique_kliff_0090_0120_player_00000.wav")));
    }

    #[test]
    fn reports_silence_as_unacceptable() {
        let report = quality_report(&[0.0; 2048]);
        assert!(!report.is_acceptable);
        assert!(report.issues.iter().any(|issue| issue.contains("mudo")));
    }

    #[test]
    fn reads_float_wav_samples_without_pcm16_byte_misinterpretation() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("float.wav");
        write_test_wav_mono_f32(&path, 24_000, &[0.25, -0.5, 0.75]);

        let samples = read_wav_mono_f32(&path).expect("read wav");

        assert_eq!(samples.len(), 3);
        assert!((samples[0] - 0.25).abs() <= f32::EPSILON);
        assert!((samples[1] + 0.5).abs() <= f32::EPSILON);
        assert!((samples[2] - 0.75).abs() <= f32::EPSILON);
    }

    #[test]
    fn reads_pcm16_wav_samples_through_hound() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("pcm16.wav");
        write_test_wav_mono_i16(&path, 24_000, &[16_384, -16_384, 0]);

        let samples = read_wav_mono_f32(&path).expect("read wav");

        assert_eq!(samples.len(), 3);
        assert!((samples[0] - 0.5).abs() < 0.001);
        assert!((samples[1] + 0.5).abs() < 0.001);
        assert_eq!(samples[2], 0.0);
    }

    #[cfg(feature = "ml")]
    #[test]
    fn extracts_short_reference_from_first_active_window_at_tts_rate() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("source.wav");
        let mut samples = vec![0.0; TTS_SAMPLE_RATE as usize];
        samples.extend(vec![0.25; (TTS_SAMPLE_RATE as f32 * 9.0) as usize]);
        write_pcm16_wav_mono(&path, TTS_SAMPLE_RATE, &samples).expect("write source wav");

        let reference = short_reference_waveform(&path).expect("short reference");

        assert_eq!(reference.sample_rate, TTS_SAMPLE_RATE);
        assert!((reference.start_seconds - 1.0).abs() < 0.01);
        assert!((reference.duration_seconds - SHORT_REFERENCE_TARGET_SECONDS).abs() < 0.01);
        assert!((reference.source_duration_seconds - 10.0).abs() < 0.01);
    }

    #[cfg(feature = "ml")]
    #[test]
    fn timing_profile_preserves_guarded_tail_without_truncating_voice() {
        let sample_rate = 1000;
        let mut samples = vec![0.0; 100];
        samples.extend(vec![0.4; 700]);
        samples.extend(vec![0.0; 200]);

        let profile = audio_timing_profile_from_samples(&samples, sample_rate, 200);

        assert_eq!(profile.total_ms, 1000);
        assert_eq!(profile.leading_silence_ms, 100);
        assert_eq!(profile.trailing_silence_ms, 200);
        assert_eq!(profile.voice_ms, 700);
    }
}
