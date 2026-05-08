use crate::{
    audio, config,
    error::{AppError, AppResult},
    jobs,
    speech::models::resolve_speech_model_paths,
    state::AppState,
};
use dublagem_domain::{
    AppConfig, AudioFileEntry, AudioMetadata, DubbingRequest, JobId, QualityReport,
    TranslationRequest, TranslationResult,
};
use sha2::Digest;
use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn load_config(app: AppHandle) -> AppResult<AppConfig> {
    config::load_config(&app)
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: AppConfig) -> AppResult<AppConfig> {
    config::save_config(&app, &config)
}

#[tauri::command]
pub fn scan_audio_folder(
    input_dir: PathBuf,
    output_dir: Option<PathBuf>,
) -> AppResult<Vec<AudioFileEntry>> {
    audio::scan_audio_folder(&input_dir, output_dir.as_deref())
}

#[tauri::command]
pub fn get_audio_metadata(path: PathBuf) -> AppResult<AudioMetadata> {
    audio::get_audio_metadata(&path)
}

#[tauri::command]
pub fn prepare_audio_preview(app: AppHandle, source: PathBuf) -> AppResult<PathBuf> {
    if !source.is_file() || !audio::is_audio_file(&source) {
        return Err(AppError::InvalidPath(source));
    }

    let metadata = std::fs::metadata(&source)?;
    let preview_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| AppError::InvalidConfig(error.to_string()))?
        .join("audio-preview");
    std::fs::create_dir_all(&preview_dir)?;

    let target = preview_dir.join(preview_file_name(&source, &metadata));
    let should_copy = std::fs::metadata(&target)
        .map(|target_metadata| target_metadata.len() != metadata.len())
        .unwrap_or(true);
    if should_copy {
        std::fs::copy(&source, &target)?;
    }

    Ok(target)
}

#[tauri::command]
pub fn inspect_audio_quality(path: PathBuf) -> AppResult<QualityReport> {
    let samples = audio::read_wav_mono_f32(&path)?;
    Ok(audio::quality_report(&samples))
}

#[tauri::command]
pub async fn transcribe_audio(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
    source_language: dublagem_domain::LanguageCode,
    target_language: dublagem_domain::LanguageCode,
) -> AppResult<dublagem_domain::TranscriptionResult> {
    let config = config::load_config(&app)?;
    let model_paths = resolve_speech_model_paths(config.model_dir.as_deref())?;
    let transcriber = state
        .speech
        .transcriber(model_paths.whisper_model_path)
        .await?;
    transcriber
        .transcribe(&path, source_language, target_language)
        .await
}

#[tauri::command]
pub async fn translate_text(
    state: State<'_, AppState>,
    request: TranslationRequest,
) -> AppResult<TranslationResult> {
    state.translator.translate(request).await
}

#[tauri::command]
pub async fn start_dubbing_job(
    app: AppHandle,
    state: State<'_, AppState>,
    request: DubbingRequest,
) -> AppResult<JobId> {
    jobs::start_dubbing_job(app, state, request).await
}

#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>, job_id: JobId) -> AppResult<()> {
    state.jobs.cancel(job_id).await
}

#[tauri::command]
pub fn approve_file(source: PathBuf, approved_dir: PathBuf) -> AppResult<PathBuf> {
    copy_to_dir(source, approved_dir)
}

#[tauri::command]
pub fn reject_file(source: PathBuf, rejected_dir: PathBuf) -> AppResult<PathBuf> {
    copy_to_dir(source, rejected_dir)
}

#[tauri::command]
pub async fn generate_voice_pool(
    app: AppHandle,
    state: State<'_, AppState>,
    output_dir: PathBuf,
) -> AppResult<Vec<PathBuf>> {
    let config = config::load_config(&app)?;
    let model_paths = resolve_speech_model_paths(config.model_dir.as_deref())?;
    let synthesizer = state
        .speech
        .synthesizer(model_paths.omnivoice_model_dir)
        .await?;
    synthesizer.generate_voice_pool(&output_dir).await
}

fn copy_to_dir(source: PathBuf, target_dir: PathBuf) -> AppResult<PathBuf> {
    if !source.is_file() {
        return Err(AppError::InvalidPath(source));
    }
    std::fs::create_dir_all(&target_dir)?;
    let file_name = source
        .file_name()
        .ok_or_else(|| AppError::InvalidPath(source.clone()))?;
    let target = target_dir.join(file_name);
    std::fs::copy(&source, &target)?;
    Ok(target)
}

fn preview_file_name(source: &Path, metadata: &std::fs::Metadata) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(source.to_string_lossy().as_bytes());
    hasher.update(metadata.len().to_le_bytes());
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
            hasher.update(duration.as_nanos().to_le_bytes());
        }
    }
    let digest = hasher.finalize();
    let fingerprint = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(sanitize_component)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "audio".to_string());
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_component)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "preview".to_string());

    format!("{stem}-{fingerprint}.{extension}")
}

fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || matches!(char, '-' | '_') {
                char
            } else {
                '_'
            }
        })
        .collect()
}
