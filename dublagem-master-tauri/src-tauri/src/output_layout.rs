use crate::error::AppResult;
use dublagem_domain::{AudioFileStatus, AudioMetadata, OMNIVOICE_MAX_SYNTHESIS_SECONDS};
use std::path::{Path, PathBuf};

pub const IGNORED_DIR_NAME: &str = "Ignorados";
pub const REJECTED_DIR_NAME: &str = "Reprovados";
pub const APPROVED_DIR_NAME: &str = "Aprovados";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputArtifact {
    pub status: AudioFileStatus,
    pub path: Option<PathBuf>,
}

impl OutputArtifact {
    pub const fn pending() -> Self {
        Self {
            status: AudioFileStatus::Pending,
            path: None,
        }
    }
}

pub fn ensure_output_layout(output_dir: &Path) -> AppResult<()> {
    std::fs::create_dir_all(ignored_dir(output_dir))?;
    std::fs::create_dir_all(rejected_dir(output_dir))?;
    std::fs::create_dir_all(approved_dir(output_dir))?;
    Ok(())
}

pub fn ensure_output_parent(output_path: &Path) -> AppResult<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn approved_output_path_for_metadata(
    output_dir: &Path,
    file_name: &str,
    metadata: Option<&AudioMetadata>,
) -> PathBuf {
    approved_output_path(
        output_dir,
        file_name,
        chunk_count_for_duration_seconds(metadata.and_then(|item| item.duration_seconds)),
    )
}

pub fn approved_output_path(output_dir: &Path, file_name: &str, chunk_count: usize) -> PathBuf {
    approved_chunk_dir(output_dir, chunk_count.max(1)).join(file_name)
}

pub fn ignored_output_path(output_dir: &Path, file_name: &str) -> PathBuf {
    ignored_dir(output_dir).join(file_name)
}

pub fn rejected_output_path(output_dir: &Path, file_name: &str) -> PathBuf {
    rejected_dir(output_dir).join(file_name)
}

pub fn output_artifact_for_source(
    output_dir: &Path,
    file_name: &str,
    metadata: Option<&AudioMetadata>,
) -> OutputArtifact {
    let rejected_path = rejected_output_path(output_dir, file_name);
    if rejected_path.exists() {
        return OutputArtifact {
            status: AudioFileStatus::Rejected,
            path: Some(rejected_path),
        };
    }

    let ignored_path = ignored_output_path(output_dir, file_name);
    if ignored_path.exists() {
        return OutputArtifact {
            status: AudioFileStatus::Ignored,
            path: Some(ignored_path),
        };
    }

    for approved_path in approved_output_candidates(output_dir, file_name, metadata) {
        if approved_path.exists() {
            return OutputArtifact {
                status: AudioFileStatus::Dubbed,
                path: Some(approved_path),
            };
        }
    }

    OutputArtifact::pending()
}

pub fn copy_to_ignored(source: &Path, output_dir: &Path, file_name: &str) -> AppResult<PathBuf> {
    let target = ignored_output_path(output_dir, file_name);
    copy_output_artifact(source, target)
}

pub fn copy_to_rejected(source: &Path, output_dir: &Path) -> AppResult<PathBuf> {
    let file_name = source
        .file_name()
        .ok_or_else(|| crate::error::AppError::InvalidPath(source.to_path_buf()))?
        .to_string_lossy()
        .to_string();
    let target = rejected_output_path(output_dir, &file_name);
    copy_output_artifact(source, target)
}

pub fn move_generated_to_rejected(
    generated_output: &Path,
    output_dir: &Path,
    file_name: &str,
) -> AppResult<PathBuf> {
    let target = rejected_output_path(output_dir, file_name);
    move_output_artifact(generated_output, target)
}

pub fn remove_ignored_and_rejected(output_dir: &Path, file_name: &str) -> AppResult<()> {
    remove_file_if_exists(&ignored_output_path(output_dir, file_name))?;
    remove_file_if_exists(&rejected_output_path(output_dir, file_name))?;
    Ok(())
}

pub fn remove_approved_outputs(
    output_dir: &Path,
    file_name: &str,
    metadata: Option<&AudioMetadata>,
) -> AppResult<()> {
    for path in approved_output_candidates(output_dir, file_name, metadata) {
        remove_file_if_exists(&path)?;
    }
    Ok(())
}

pub fn chunk_count_for_duration_seconds(duration_seconds: Option<f64>) -> usize {
    let Some(duration_seconds) =
        duration_seconds.filter(|seconds| seconds.is_finite() && *seconds > 0.0)
    else {
        return 1;
    };

    (duration_seconds / f64::from(OMNIVOICE_MAX_SYNTHESIS_SECONDS))
        .ceil()
        .max(1.0) as usize
}

fn approved_output_candidates(
    output_dir: &Path,
    file_name: &str,
    metadata: Option<&AudioMetadata>,
) -> Vec<PathBuf> {
    let preferred = approved_output_path_for_metadata(output_dir, file_name, metadata);
    let mut candidates = vec![preferred.clone()];

    for chunk_dir in existing_approved_chunk_dirs(output_dir) {
        let candidate = chunk_dir.join(file_name);
        if candidate != preferred {
            candidates.push(candidate);
        }
    }

    candidates
}

fn existing_approved_chunk_dirs(output_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(approved_dir(output_dir)) else {
        return Vec::new();
    };

    let mut dirs = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }

            let name = path.file_name()?.to_str()?;
            let chunk_count = chunk_count_from_folder_name(name)?;
            Some((chunk_count, path))
        })
        .collect::<Vec<_>>();
    dirs.sort_by_key(|(chunk_count, _)| *chunk_count);
    dirs.into_iter().map(|(_, path)| path).collect()
}

fn copy_output_artifact(source: &Path, target: PathBuf) -> AppResult<PathBuf> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, &target)?;
    Ok(target)
}

fn move_output_artifact(source: &Path, target: PathBuf) -> AppResult<PathBuf> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    remove_file_if_exists(&target)?;

    match std::fs::rename(source, &target) {
        Ok(()) => Ok(target),
        Err(rename_error) => {
            std::fs::copy(source, &target).map_err(|copy_error| {
                crate::error::AppError::Io(format!(
                    "falha ao mover artefato de saída de {} para {}: {rename_error}; fallback de cópia falhou: {copy_error}",
                    source.display(),
                    target.display()
                ))
            })?;
            std::fs::remove_file(source)?;
            Ok(target)
        }
    }
}

fn ignored_dir(output_dir: &Path) -> PathBuf {
    output_dir.join(IGNORED_DIR_NAME)
}

fn rejected_dir(output_dir: &Path) -> PathBuf {
    output_dir.join(REJECTED_DIR_NAME)
}

fn approved_dir(output_dir: &Path) -> PathBuf {
    output_dir.join(APPROVED_DIR_NAME)
}

fn approved_chunk_dir(output_dir: &Path, chunk_count: usize) -> PathBuf {
    approved_dir(output_dir).join(chunk_folder_name(chunk_count))
}

fn chunk_folder_name(chunk_count: usize) -> String {
    format!("Chunk {}", chunk_count.max(1))
}

fn chunk_count_from_folder_name(folder_name: &str) -> Option<usize> {
    folder_name
        .strip_prefix("Chunk ")
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn remove_file_if_exists(path: &Path) -> AppResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_fixed_output_layout_roots_without_fixed_chunk_cap() {
        let output_dir = tempfile::tempdir().expect("output tempdir");

        ensure_output_layout(output_dir.path()).expect("layout");

        assert!(output_dir.path().join(IGNORED_DIR_NAME).is_dir());
        assert!(output_dir.path().join(REJECTED_DIR_NAME).is_dir());
        assert!(output_dir.path().join(APPROVED_DIR_NAME).is_dir());
        assert!(!output_dir
            .path()
            .join(APPROVED_DIR_NAME)
            .join("Chunk 4")
            .exists());
    }

    #[test]
    fn creates_parent_for_arbitrary_chunk_output() {
        let output_dir = tempfile::tempdir().expect("output tempdir");
        let path = approved_output_path(output_dir.path(), "line.wav", 12);

        ensure_output_parent(&path).expect("output parent");

        assert!(output_dir
            .path()
            .join(APPROVED_DIR_NAME)
            .join("Chunk 12")
            .is_dir());
    }

    #[test]
    fn resolves_approved_output_under_chunk_folder() {
        let path = approved_output_path(
            Path::new(r"E:\audio\saida"),
            "unique_kliff_0090_0120_player_00000.wav",
            2,
        );

        assert!(path.ends_with(
            Path::new(APPROVED_DIR_NAME)
                .join("Chunk 2")
                .join("unique_kliff_0090_0120_player_00000.wav")
        ));
    }

    #[test]
    fn derives_chunk_count_from_duration_boundaries() {
        assert_eq!(chunk_count_for_duration_seconds(None), 1);
        assert_eq!(chunk_count_for_duration_seconds(Some(0.0)), 1);
        assert_eq!(chunk_count_for_duration_seconds(Some(30.0)), 1);
        assert_eq!(chunk_count_for_duration_seconds(Some(30.01)), 2);
        assert_eq!(chunk_count_for_duration_seconds(Some(60.0)), 2);
        assert_eq!(chunk_count_for_duration_seconds(Some(90.0)), 3);
        assert_eq!(chunk_count_for_duration_seconds(Some(120.0)), 4);
        assert_eq!(chunk_count_for_duration_seconds(Some(300.0)), 10);
    }

    #[test]
    fn finds_existing_approved_artifacts_in_any_chunk_folder() {
        let output_dir = tempfile::tempdir().expect("output tempdir");
        let file_name = "line_any_chunk.wav";
        let path = approved_output_path(output_dir.path(), file_name, 12);
        ensure_output_parent(&path).expect("output parent");
        std::fs::write(&path, b"dubbed").expect("dubbed file");

        let artifact = output_artifact_for_source(output_dir.path(), file_name, None);

        assert_eq!(artifact.status, AudioFileStatus::Dubbed);
        assert_eq!(artifact.path.as_deref(), Some(path.as_path()));
    }

    #[test]
    fn moves_generated_rejection_artifact_without_copying_source_audio() {
        let output_dir = tempfile::tempdir().expect("output tempdir");
        let file_name = "line.wav";
        let generated_path = approved_output_path(output_dir.path(), file_name, 6);
        let stale_rejected_path = rejected_output_path(output_dir.path(), file_name);

        ensure_output_parent(&generated_path).expect("approved parent");
        ensure_output_parent(&stale_rejected_path).expect("rejected parent");
        std::fs::write(&generated_path, b"generated portuguese audio").expect("generated output");
        std::fs::write(&stale_rejected_path, b"stale source audio").expect("stale rejected");

        let rejected_path =
            move_generated_to_rejected(&generated_path, output_dir.path(), file_name)
                .expect("move generated rejection");

        assert_eq!(rejected_path, stale_rejected_path);
        assert!(!generated_path.exists());
        assert_eq!(
            std::fs::read(rejected_path).expect("rejected artifact"),
            b"generated portuguese audio"
        );
    }
}
