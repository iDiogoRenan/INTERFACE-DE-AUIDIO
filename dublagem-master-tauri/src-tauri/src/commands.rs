use crate::{
    audio, config,
    error::{AppError, AppResult},
    jobs,
    speech::VoiceSynthesizer,
    state::AppState,
    translation::TranslationProvider,
};
use dublagem_domain::{
    AppConfig, AudioFileEntry, AudioMetadata, DubbingRequest, JobId, QualityReport,
    TranslationRequest, TranslationResult,
};
use std::path::PathBuf;
use tauri::{AppHandle, State};

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
pub fn inspect_audio_quality(path: PathBuf) -> AppResult<QualityReport> {
    let samples = audio::read_wav_mono_f32(&path)?;
    Ok(audio::quality_report(&samples))
}

#[tauri::command]
pub async fn transcribe_audio(
    state: State<'_, AppState>,
    path: PathBuf,
    source_language: dublagem_domain::LanguageCode,
    target_language: dublagem_domain::LanguageCode,
) -> AppResult<dublagem_domain::TranscriptionResult> {
    crate::speech::Transcriber::transcribe(
        state.transcriber.as_ref(),
        &path,
        source_language,
        target_language,
    )
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
    state: State<'_, AppState>,
    output_dir: PathBuf,
) -> AppResult<Vec<PathBuf>> {
    state.synthesizer.generate_voice_pool(&output_dir).await
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
