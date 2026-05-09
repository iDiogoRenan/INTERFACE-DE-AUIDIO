use crate::error::{AppError, AppResult};
use dublagem_domain::{NativeSynthesisSettings, ProjectMetadata, OMNIVOICE_NATIVE_TAGS};
use std::path::{Path, PathBuf};

pub const PROJECT_METADATA_FILE: &str = "nsg_dub_project.json";

pub fn load_project_metadata(output_dir: &Path) -> AppResult<ProjectMetadata> {
    if !output_dir.is_dir() {
        return Ok(ProjectMetadata::v1());
    }

    let path = metadata_path(output_dir);
    if !path.is_file() {
        return Ok(ProjectMetadata::v1());
    }

    let payload = std::fs::read_to_string(path)?;
    let mut metadata: ProjectMetadata = serde_json::from_str(&payload)?;
    if metadata.version == 0 {
        metadata.version = 1;
    }
    validate_project_metadata(&metadata)?;
    Ok(metadata)
}

pub fn save_project_metadata(
    output_dir: &Path,
    mut metadata: ProjectMetadata,
) -> AppResult<ProjectMetadata> {
    std::fs::create_dir_all(output_dir)?;
    metadata.version = 1;
    validate_project_metadata(&metadata)?;

    let path = metadata_path(output_dir);
    let temp_path = path.with_extension("json.tmp");
    let payload = serde_json::to_string_pretty(&metadata)?;
    std::fs::write(&temp_path, payload)?;
    std::fs::rename(temp_path, path)?;
    Ok(metadata)
}

pub fn validate_project_metadata(metadata: &ProjectMetadata) -> AppResult<()> {
    for (file_key, file) in &metadata.files {
        if file_key.trim().is_empty() {
            return Err(AppError::InvalidConfig(
                "metadados de projeto contem chave de arquivo vazia".to_string(),
            ));
        }
        if let Some(target_text) = &file.target_text {
            validate_text_native_tags(target_text)?;
        }

        for (line_key, line) in &file.lines {
            if line_key.parse::<usize>().is_err() {
                return Err(AppError::InvalidConfig(format!(
                    "linha invalida em metadados de {file_key}: {line_key}"
                )));
            }
            validate_native_tags(&line.tags)?;
            line.settings.validate().map_err(AppError::InvalidConfig)?;
        }
    }
    Ok(())
}

pub fn validate_settings(settings: &NativeSynthesisSettings) -> AppResult<()> {
    settings.validate().map_err(AppError::InvalidConfig)
}

pub fn validate_text_native_tags(text: &str) -> AppResult<()> {
    for tag in bracketed_lowercase_tags(text) {
        if !OMNIVOICE_NATIVE_TAGS.contains(&tag.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "tag OmniVoice nao suportada: {tag}"
            )));
        }
    }
    Ok(())
}

pub fn validate_native_tags(tags: &[String]) -> AppResult<()> {
    for tag in tags {
        if !OMNIVOICE_NATIVE_TAGS.contains(&tag.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "tag OmniVoice nao suportada: {tag}"
            )));
        }
    }
    Ok(())
}

fn metadata_path(output_dir: &Path) -> PathBuf {
    output_dir.join(PROJECT_METADATA_FILE)
}

fn bracketed_lowercase_tags(text: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut current = String::new();
    let mut inside = false;

    for character in text.chars() {
        match (inside, character) {
            (false, '[') => {
                inside = true;
                current.clear();
                current.push(character);
            }
            (true, ']') => {
                current.push(character);
                if is_lowercase_tag_candidate(&current) {
                    tags.push(current.clone());
                }
                inside = false;
                current.clear();
            }
            (true, value) => {
                current.push(value);
                if current.len() > 80 {
                    inside = false;
                    current.clear();
                }
            }
            (false, _) => {}
        }
    }

    tags
}

fn is_lowercase_tag_candidate(value: &str) -> bool {
    let content = value
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or_default();
    !content.is_empty()
        && content.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        && content
            .chars()
            .next()
            .map(|character| character.is_ascii_lowercase())
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dublagem_domain::{ProjectFileMetadata, ProjectLineMetadata};

    #[test]
    fn rejects_unknown_lowercase_nonverbal_tags() {
        let error = validate_text_native_tags("Texto [angry] invalido.").expect_err("invalid tag");

        assert!(error.to_string().contains("[angry]"));
    }

    #[test]
    fn allows_native_tags_and_uppercase_pronunciation_hints() {
        validate_text_native_tags("[sigh] He plays [B EY1 S].").expect("native tags");
    }

    #[test]
    fn persists_project_metadata_with_atomic_shape() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut metadata = ProjectMetadata::v1();
        metadata.files.insert(
            "line.wav".to_string(),
            ProjectFileMetadata {
                source_text: Some("Hello".to_string()),
                target_text: Some("[sigh] Ola".to_string()),
                baseline_source_text: Some("Hello".to_string()),
                baseline_target_text: Some("Ola".to_string()),
                lines: [(
                    "0".to_string(),
                    ProjectLineMetadata {
                        tags: vec!["[sigh]".to_string()],
                        ..ProjectLineMetadata::default()
                    },
                )]
                .into_iter()
                .collect(),
            },
        );

        save_project_metadata(temp_dir.path(), metadata).expect("save metadata");
        let loaded = load_project_metadata(temp_dir.path()).expect("load metadata");

        assert_eq!(loaded.version, 1);
        assert!(loaded.files.contains_key("line.wav"));
    }
}
