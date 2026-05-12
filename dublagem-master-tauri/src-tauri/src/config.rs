use crate::error::{AppError, AppResult};
use dublagem_domain::AppConfig;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const CONFIG_FILE_NAME: &str = "config.json";

pub fn config_path(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| AppError::InvalidConfig(error.to_string()))?;
    Ok(dir.join(CONFIG_FILE_NAME))
}

pub fn load_config(app: &AppHandle) -> AppResult<AppConfig> {
    let path = config_path(app)?;
    if !path.exists() {
        return Ok(with_discovered_models(app, AppConfig::default()));
    }

    let payload = std::fs::read_to_string(path)?;
    Ok(with_discovered_models(app, serde_json::from_str(&payload)?))
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> AppResult<AppConfig> {
    let config = with_discovered_models(app, config.clone());
    let path = config_path(app)?;
    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidConfig("caminho de configuração sem diretório".to_string())
    })?;
    std::fs::create_dir_all(parent)?;
    let payload = serde_json::to_string_pretty(&config)?;
    std::fs::write(path, payload)?;
    Ok(config)
}

fn with_discovered_models(app: &AppHandle, mut config: AppConfig) -> AppConfig {
    if config.model_dir.is_none() {
        config.model_dir = crate::speech::models::discover_model_dir_for_app(app);
    }
    config.normalize_model_presets()
}

#[cfg(test)]
mod tests {
    use dublagem_domain::{DubbingMode, DubbingOptions, LanguageCode};

    #[test]
    fn defaults_match_legacy_application() {
        let options = DubbingOptions::default();
        assert_eq!(options.source_language, LanguageCode::En);
        assert_eq!(options.target_language, LanguageCode::Pt);
        assert_eq!(options.mode, DubbingMode::Classico);
        assert_eq!(options.pad_ms, 200);
        assert_eq!(options.omni_temperature, 0.0);
        assert_eq!(options.max_synthesis_chunks, 1);
        assert!(!options.preserve_sentence_boundaries);
        assert_eq!(options.native_synthesis.num_step, 48);
        assert_eq!(options.native_synthesis.position_temperature, 1.0);
        assert!(options.native_synthesis.denoise);
        assert!(!options.native_synthesis.match_source_loudness);
    }
}
