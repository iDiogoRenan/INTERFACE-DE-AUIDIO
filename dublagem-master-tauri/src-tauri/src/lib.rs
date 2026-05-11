mod audio;
mod commands;
mod config;
mod error;
mod jobs;
mod output_layout;
mod project_metadata;
mod speech;
mod state;
mod text;
mod translation;

use commands::{
    approve_file, cancel_job, generate_voice_pool, get_audio_metadata, inspect_audio_quality,
    load_config, load_project_metadata, prepare_audio_preview, preview_synthesis_line, reject_file,
    save_config, save_project_metadata, scan_audio_folder, start_dubbing_job, transcribe_audio,
    translate_text,
};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            load_project_metadata,
            save_project_metadata,
            scan_audio_folder,
            get_audio_metadata,
            prepare_audio_preview,
            inspect_audio_quality,
            transcribe_audio,
            translate_text,
            start_dubbing_job,
            preview_synthesis_line,
            cancel_job,
            approve_file,
            reject_file,
            generate_voice_pool
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
