use crate::{
    error::{AppError, AppResult},
    speech::missing_model_error,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};
use tauri::{path::BaseDirectory, AppHandle, Manager};

const MODEL_MANIFEST_FILE_NAME: &str = "MODEL_MANIFEST.json";
const BUNDLED_MODELS_RESOURCE_DIR: &str = "models";
const WHISPER_MODEL_ID: &str = "whisper-large-v3-ggml";
const WHISPER_ENGINE: &str = "whisper-rs";
const WHISPER_FALLBACK_PATH: &str = "whisper/ggml-large-v3.bin";
const WHISPER_VAD_MODEL_ID: &str = "whisper-vad-silero-v6.2.0-ggml";
const WHISPER_VAD_ENGINE: &str = "whisper-rs-vad";
const WHISPER_VAD_FALLBACK_PATH: &str = "whisper/ggml-silero-v6.2.0.bin";
const OMNIVOICE_MODEL_ID: &str = "omnivoice-candle";
const OMNIVOICE_ENGINE: &str = "omnivoice-candle";
const OMNIVOICE_FALLBACK_PATH: &str = "omnivoice";
const OMNIVOICE_RUNTIME_MANIFEST_FILE_NAME: &str = "omnivoice.artifacts.json";
const OMNIVOICE_RUNTIME_MANIFEST_JSON: &str = include_str!(
    "../../../vendor/omnivoice-rs/crates/omnivoice-infer/assets/omnivoice.artifacts.json"
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechModelPaths {
    pub whisper_model_path: PathBuf,
    pub whisper_vad_model_path: PathBuf,
    pub omnivoice_model_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelManifest {
    schema_version: u32,
    models: Vec<ModelManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelManifestEntry {
    id: String,
    engine: String,
    path: PathBuf,
    sha256: Option<String>,
    files: Option<Vec<ModelManifestFile>>,
}

#[derive(Debug, Deserialize)]
struct ModelManifestFile {
    path: PathBuf,
    sha256: String,
}

pub fn resolve_speech_model_paths(model_dir: Option<&Path>) -> AppResult<SpeechModelPaths> {
    let model_dir = model_dir.map(Path::to_path_buf).or_else(discover_model_dir);

    let Some(model_dir) = model_dir else {
        return Err(AppError::SpeechEngineUnavailable(
            "modelos locais não provisionados. Execute a migração para dublagem-master-tauri/models antes de dublar.".to_string(),
        ));
    };

    if !model_dir.is_dir() {
        return Err(AppError::InvalidPath(model_dir.to_path_buf()));
    }

    let manifest_path = model_dir.join(MODEL_MANIFEST_FILE_NAME);
    let paths = if manifest_path.is_file() {
        resolve_from_manifest(&manifest_path)?
    } else {
        SpeechModelPaths {
            whisper_model_path: model_dir.join(WHISPER_FALLBACK_PATH),
            whisper_vad_model_path: model_dir.join(WHISPER_VAD_FALLBACK_PATH),
            omnivoice_model_dir: model_dir.join(OMNIVOICE_FALLBACK_PATH),
        }
    };

    ensure_file(&paths.whisper_model_path, "whisper-rs ggml large-v3")?;
    ensure_file(
        &paths.whisper_vad_model_path,
        "whisper-rs Silero VAD v6.2.0",
    )?;
    ensure_dir(&paths.omnivoice_model_dir, "OmniVoice Candle")?;
    ensure_omnivoice_runtime_manifest(&paths.omnivoice_model_dir)?;
    Ok(paths)
}

pub fn discover_model_dir_for_app(app: &AppHandle) -> Option<PathBuf> {
    discover_bundled_model_dir(app)
        .or_else(discover_exe_adjacent_model_dir)
        .or_else(discover_model_dir)
}

pub fn model_dir_or_discovered_for_app(
    app: &AppHandle,
    configured_model_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    model_dir_or_discovered(configured_model_dir, discover_model_dir_for_app(app))
}

pub fn model_dir_or_discovered(
    configured_model_dir: Option<PathBuf>,
    discovered_model_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    match configured_model_dir {
        Some(model_dir) if is_usable_model_dir(&model_dir) => Some(model_dir),
        Some(model_dir) => discovered_model_dir.or(Some(model_dir)),
        None => discovered_model_dir,
    }
}

pub fn discover_model_dir() -> Option<PathBuf> {
    model_dir_if_available(project_model_dir())
}

fn discover_bundled_model_dir(app: &AppHandle) -> Option<PathBuf> {
    let model_dir = app
        .path()
        .resolve(BUNDLED_MODELS_RESOURCE_DIR, BaseDirectory::Resource)
        .ok()?;
    model_dir_if_available(model_dir)
}

fn discover_exe_adjacent_model_dir() -> Option<PathBuf> {
    let model_dir = std::env::current_exe()
        .ok()?
        .parent()
        .map(|directory| directory.join(BUNDLED_MODELS_RESOURCE_DIR))?;
    model_dir_if_available(model_dir)
}

pub fn is_usable_model_dir(model_dir: &Path) -> bool {
    model_dir.is_dir()
        && (model_dir.join(MODEL_MANIFEST_FILE_NAME).is_file()
            || (model_dir.join(WHISPER_FALLBACK_PATH).is_file()
                && model_dir.join(WHISPER_VAD_FALLBACK_PATH).is_file()
                && model_dir.join(OMNIVOICE_FALLBACK_PATH).is_dir()))
}

fn model_dir_if_available(model_dir: PathBuf) -> Option<PathBuf> {
    is_usable_model_dir(&model_dir).then_some(model_dir)
}

pub fn project_model_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|project_dir| project_dir.join("models"))
        .unwrap_or_else(|| PathBuf::from("models"))
}

fn resolve_from_manifest(manifest_path: &Path) -> AppResult<SpeechModelPaths> {
    let payload = std::fs::read_to_string(manifest_path)?;
    let manifest: ModelManifest = serde_json::from_str(&payload)?;
    if manifest.schema_version != 1 {
        return Err(AppError::InvalidConfig(format!(
            "manifesto de modelos com schemaVersion {} não suportado",
            manifest.schema_version
        )));
    }

    let base_dir = manifest_path.parent().ok_or_else(|| {
        AppError::InvalidConfig("manifesto de modelos sem diretorio base".to_string())
    })?;
    let whisper = resolve_entry(&manifest.models, base_dir, WHISPER_MODEL_ID, WHISPER_ENGINE)?;
    let whisper_vad = resolve_entry(
        &manifest.models,
        base_dir,
        WHISPER_VAD_MODEL_ID,
        WHISPER_VAD_ENGINE,
    )?;
    let omnivoice = resolve_entry(
        &manifest.models,
        base_dir,
        OMNIVOICE_MODEL_ID,
        OMNIVOICE_ENGINE,
    )?;

    Ok(SpeechModelPaths {
        whisper_model_path: whisper,
        whisper_vad_model_path: whisper_vad,
        omnivoice_model_dir: omnivoice,
    })
}

fn resolve_entry(
    entries: &[ModelManifestEntry],
    base_dir: &Path,
    id: &str,
    engine: &str,
) -> AppResult<PathBuf> {
    let entry = entries
        .iter()
        .find(|candidate| candidate.id == id && candidate.engine == engine)
        .ok_or_else(|| {
            AppError::InvalidConfig(format!(
                "manifesto de modelos sem entrada {id} para {engine}"
            ))
        })?;
    let path = if entry.path.is_absolute() {
        entry.path.clone()
    } else {
        base_dir.join(&entry.path)
    };

    if path.is_file() {
        verify_file_hash(&path, entry.sha256.as_deref())?;
    }
    if path.is_dir() {
        verify_manifest_files(base_dir, entry.files.as_deref())?;
    }

    Ok(path)
}

fn verify_manifest_files(base_dir: &Path, files: Option<&[ModelManifestFile]>) -> AppResult<()> {
    let Some(files) = files else {
        return Ok(());
    };

    for file in files {
        let path = if file.path.is_absolute() {
            file.path.clone()
        } else {
            base_dir.join(&file.path)
        };
        if !path.is_file() {
            return Err(AppError::InvalidPath(path));
        }
        verify_file_hash(&path, Some(&file.sha256))?;
    }

    Ok(())
}

fn verify_file_hash(path: &Path, expected_sha256: Option<&str>) -> AppResult<()> {
    let Some(expected_sha256) = expected_sha256 else {
        return Ok(());
    };
    if !is_sha256(expected_sha256) {
        return Err(AppError::InvalidConfig(format!(
            "sha256 invalido no manifesto para {}",
            path.display()
        )));
    }

    let mut hasher = Sha256::new();
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let actual = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != expected_sha256.to_ascii_lowercase() {
        return Err(AppError::InvalidConfig(format!(
            "sha256 divergente para {}",
            path.display()
        )));
    }

    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|char| char.is_ascii_hexdigit())
}

fn ensure_file(path: &Path, model: &str) -> AppResult<()> {
    if path.is_file() {
        return Ok(());
    }
    Err(missing_model_error(model, path))
}

fn ensure_dir(path: &Path, model: &str) -> AppResult<()> {
    if path.is_dir() {
        return Ok(());
    }
    Err(missing_model_error(model, path))
}

fn ensure_omnivoice_runtime_manifest(model_dir: &Path) -> AppResult<()> {
    let manifest_path = model_dir.join(OMNIVOICE_RUNTIME_MANIFEST_FILE_NAME);
    if manifest_path.is_file() {
        return Ok(());
    }

    std::fs::write(manifest_path, OMNIVOICE_RUNTIME_MANIFEST_JSON)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_conventional_model_layout() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let whisper_dir = temp_dir.path().join("whisper");
        let omnivoice_dir = temp_dir.path().join("omnivoice");
        std::fs::create_dir_all(&whisper_dir).expect("whisper dir");
        std::fs::create_dir_all(&omnivoice_dir).expect("omnivoice dir");
        std::fs::write(whisper_dir.join("ggml-large-v3.bin"), b"model").expect("whisper model");
        std::fs::write(whisper_dir.join("ggml-silero-v6.2.0.bin"), b"vad").expect("vad model");
        std::fs::write(omnivoice_dir.join("model.safetensors"), b"model").expect("omni model");

        let paths = resolve_speech_model_paths(Some(temp_dir.path())).expect("model paths");

        assert_eq!(
            paths.whisper_model_path,
            temp_dir.path().join("whisper/ggml-large-v3.bin")
        );
        assert_eq!(
            paths.whisper_vad_model_path,
            temp_dir.path().join("whisper/ggml-silero-v6.2.0.bin")
        );
        assert_eq!(paths.omnivoice_model_dir, omnivoice_dir);
    }

    #[test]
    fn validates_manifest_file_hashes() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let whisper_path = temp_dir.path().join("ggml-large-v3.bin");
        let vad_path = temp_dir.path().join("ggml-silero-v6.2.0.bin");
        let omnivoice_dir = temp_dir.path().join("omnivoice");
        std::fs::create_dir_all(&omnivoice_dir).expect("omnivoice dir");
        std::fs::write(&whisper_path, b"model").expect("whisper model");
        std::fs::write(&vad_path, b"vad").expect("vad model");
        std::fs::write(omnivoice_dir.join("model.safetensors"), b"model").expect("omni model");
        let sha256 = format!("{:x}", Sha256::digest(b"model"));
        let vad_sha256 = format!("{:x}", Sha256::digest(b"vad"));
        std::fs::write(
            temp_dir.path().join(MODEL_MANIFEST_FILE_NAME),
            format!(
                r#"{{
  "schemaVersion": 1,
  "models": [
    {{
      "id": "whisper-large-v3-ggml",
      "engine": "whisper-rs",
      "path": "ggml-large-v3.bin",
      "sha256": "{sha256}"
    }},
    {{
      "id": "whisper-vad-silero-v6.2.0-ggml",
      "engine": "whisper-rs-vad",
      "path": "ggml-silero-v6.2.0.bin",
      "sha256": "{vad_sha256}"
    }},
    {{
      "id": "omnivoice-candle",
      "engine": "omnivoice-candle",
      "path": "omnivoice"
    }}
  ]
}}"#
            ),
        )
        .expect("manifest");

        let paths = resolve_speech_model_paths(Some(temp_dir.path())).expect("model paths");

        assert_eq!(paths.whisper_model_path, whisper_path);
        assert_eq!(paths.whisper_vad_model_path, vad_path);
        assert_eq!(paths.omnivoice_model_dir, omnivoice_dir);
    }

    #[test]
    fn reports_missing_configuration_before_engine_creation() {
        let missing_dir = tempfile::tempdir().expect("tempdir");
        let error =
            resolve_speech_model_paths(Some(missing_dir.path())).expect_err("missing config");

        assert!(error.to_string().contains("whisper-rs ggml large-v3"));
    }

    #[test]
    fn accepts_manifested_model_dir_as_usable_without_hashing_payload() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp_dir.path().join(MODEL_MANIFEST_FILE_NAME),
            r#"{"schemaVersion":1,"models":[]}"#,
        )
        .expect("manifest");

        assert!(is_usable_model_dir(temp_dir.path()));
    }

    #[test]
    fn accepts_conventional_model_dir_as_usable_without_manifest() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let whisper_dir = temp_dir.path().join("whisper");
        let omnivoice_dir = temp_dir.path().join("omnivoice");
        std::fs::create_dir_all(&whisper_dir).expect("whisper dir");
        std::fs::create_dir_all(&omnivoice_dir).expect("omnivoice dir");
        std::fs::write(whisper_dir.join("ggml-large-v3.bin"), b"model").expect("whisper model");
        std::fs::write(whisper_dir.join("ggml-silero-v6.2.0.bin"), b"vad").expect("vad model");

        assert!(is_usable_model_dir(temp_dir.path()));
    }

    #[test]
    fn replaces_stale_model_dir_with_discovered_portable_models() {
        let discovered_models = tempfile::tempdir().expect("discovered models");
        std::fs::write(
            discovered_models.path().join(MODEL_MANIFEST_FILE_NAME),
            r#"{"schemaVersion":1,"models":[]}"#,
        )
        .expect("manifest");

        let resolved = model_dir_or_discovered(
            Some(PathBuf::from(
                r"D:\CD DUBLAGEM PROJETO\NSG Gaming Dub 1.0\models",
            )),
            Some(discovered_models.path().to_path_buf()),
        );

        assert_eq!(resolved, Some(discovered_models.path().to_path_buf()));
    }

    #[test]
    fn preserves_valid_user_selected_model_dir() {
        let selected_models = tempfile::tempdir().expect("selected models");
        let discovered_models = tempfile::tempdir().expect("discovered models");
        for directory in [selected_models.path(), discovered_models.path()] {
            std::fs::write(
                directory.join(MODEL_MANIFEST_FILE_NAME),
                r#"{"schemaVersion":1,"models":[]}"#,
            )
            .expect("manifest");
        }

        let resolved = model_dir_or_discovered(
            Some(selected_models.path().to_path_buf()),
            Some(discovered_models.path().to_path_buf()),
        );

        assert_eq!(resolved, Some(selected_models.path().to_path_buf()));
    }

    #[test]
    fn materializes_omnivoice_runtime_manifest_for_official_snapshot_layout() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let whisper_dir = temp_dir.path().join("whisper");
        let omnivoice_dir = temp_dir.path().join("omnivoice");
        std::fs::create_dir_all(&whisper_dir).expect("whisper dir");
        std::fs::create_dir_all(&omnivoice_dir).expect("omnivoice dir");
        std::fs::write(whisper_dir.join("ggml-large-v3.bin"), b"model").expect("whisper model");
        std::fs::write(whisper_dir.join("ggml-silero-v6.2.0.bin"), b"vad").expect("vad model");

        resolve_speech_model_paths(Some(temp_dir.path())).expect("model paths");

        assert!(omnivoice_dir
            .join(OMNIVOICE_RUNTIME_MANIFEST_FILE_NAME)
            .is_file());
    }
}
