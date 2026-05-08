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
        return Ok(AppConfig::default());
    }

    let payload = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&payload)?)
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> AppResult<AppConfig> {
    let path = config_path(app)?;
    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidConfig("caminho de configuracao sem diretorio".to_string())
    })?;
    std::fs::create_dir_all(parent)?;
    let payload = serde_json::to_string_pretty(config)?;
    std::fs::write(path, payload)?;
    Ok(config.clone())
}

#[cfg(test)]
mod tests {
    use dublagem_domain::{DubbingMode, DubbingOptions, LanguageCode};

    #[test]
    fn defaults_match_legacy_application() {
        let options = DubbingOptions::default();
        assert_eq!(options.source_language, LanguageCode::Auto);
        assert_eq!(options.target_language, LanguageCode::Pt);
        assert_eq!(options.mode, DubbingMode::Classico);
        assert_eq!(options.pad_ms, 200);
        assert_eq!(options.omni_temperature, 0.0);
    }
}
