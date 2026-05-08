use crate::error::{AppError, AppResult};
use dublagem_domain::{AudioFileEntry, AudioFileStatus, AudioMetadata, QualityReport};
use std::{fs::File, path::Path};
use symphonia::core::{
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

pub const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "wem", "ogg", "flac"];
const FAMILY_MARKER_TOKENS: &[&str] = &["questdialog", "narration", "player"];

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
    for entry in std::fs::read_dir(input_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !is_audio_file(&path) {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let status = status_for_file(&name, output_dir);
        entries.push(AudioFileEntry {
            family: audio_family_from_filename(&name),
            metadata: get_audio_metadata(&path).ok(),
            name,
            path,
            status,
        });
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

pub fn get_audio_metadata(path: &Path) -> AppResult<AudioMetadata> {
    let extension = extension(path);
    if extension == "wem" {
        return Err(AppError::UnsupportedCodec(
            "Wwise WEM precisa de decoder Rust validado antes de entrar no pipeline".to_string(),
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
        AppError::UnsupportedCodec("arquivo sem faixa de audio padrao".to_string())
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
    read_wav_samples_fallback(path)
}

pub fn quality_report(samples: &[f32]) -> QualityReport {
    if samples.is_empty() {
        return QualityReport {
            is_acceptable: false,
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

    if peak_amplitude <= 0.0001 {
        issues.push("Audio praticamente mudo.".to_string());
    }
    if peak_amplitude > 0.985 {
        issues.push("Audio proximo de clipping.".to_string());
    }
    if zcr_average > 0.45 {
        issues.push(format!("ZCR alto demais ({zcr_average:.2})."));
    }

    QualityReport {
        is_acceptable: issues.is_empty(),
        zcr_average,
        peak_amplitude,
        rms,
        issues,
    }
}

fn status_for_file(name: &str, output_dir: Option<&Path>) -> AudioFileStatus {
    output_dir
        .map(|dir| dir.join(name).exists())
        .filter(|exists| *exists)
        .map(|_| AudioFileStatus::Dubbed)
        .unwrap_or(AudioFileStatus::Pending)
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

fn read_wav_samples_fallback(path: &Path) -> AppResult<Vec<f32>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() <= 44 {
        return Err(AppError::UnsupportedCodec(
            "wav sem dados PCM suficientes".to_string(),
        ));
    }

    let data = bytes[44..]
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32)
        .collect::<Vec<_>>();
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn reports_silence_as_unacceptable() {
        let report = quality_report(&[0.0; 2048]);
        assert!(!report.is_acceptable);
        assert!(report.issues.iter().any(|issue| issue.contains("mudo")));
    }
}
