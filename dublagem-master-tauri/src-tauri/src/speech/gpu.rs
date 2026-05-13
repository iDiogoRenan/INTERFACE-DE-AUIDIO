#![cfg(all(feature = "ml", feature = "cuda"))]

use crate::error::{AppError, AppResult};

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
};

const MIN_CUDA_13_DRIVER_BRANCH: u32 = 580;
const MIN_CUDA_13_COMPUTE_CAPABILITY: u32 = 75;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CudaGpuReport {
    name: String,
    driver_version: String,
    compute_capability: u32,
    total_memory_mb: Option<u64>,
}

impl CudaGpuReport {
    pub(crate) fn diagnostic_line(&self) -> String {
        let memory = self
            .total_memory_mb
            .map(|megabytes| format!(", VRAM: {megabytes} MiB"))
            .unwrap_or_default();
        format!(
            "GPU: {}, driver: {}, compute capability: {}.{}{}",
            self.name,
            self.driver_version,
            self.compute_capability / 10,
            self.compute_capability % 10,
            memory
        )
    }
}

pub(crate) fn require_cuda_gpu() -> AppResult<CudaGpuReport> {
    let report = query_primary_cuda_gpu()?;
    validate_driver(&report)?;
    validate_compute_capability(&report)?;
    Ok(report)
}

fn validate_driver(report: &CudaGpuReport) -> AppResult<()> {
    let Some(branch) = parse_driver_branch(&report.driver_version) else {
        return Err(AppError::SpeechEngineUnavailable(format!(
            "Não consegui interpretar o driver NVIDIA detectado ({driver}). Atualize o driver NVIDIA e tente de novo.",
            driver = report.driver_version
        )));
    };

    if branch < MIN_CUDA_13_DRIVER_BRANCH {
        return Err(AppError::SpeechEngineUnavailable(format!(
            "Driver NVIDIA incompatível com o runtime CUDA 13.x deste pacote. Detectado: {driver}. Exigido: ramo R{minimum}+ para executar bibliotecas CUDA 13. Atualize pelo site da NVIDIA e abra o programa novamente.",
            driver = report.driver_version,
            minimum = MIN_CUDA_13_DRIVER_BRANCH
        )));
    }

    Ok(())
}

fn validate_compute_capability(report: &CudaGpuReport) -> AppResult<()> {
    if report.compute_capability < MIN_CUDA_13_COMPUTE_CAPABILITY {
        return Err(AppError::SpeechEngineUnavailable(format!(
            "GPU NVIDIA incompatível com este pacote CUDA 13.x. Detectada: {gpu} compute capability {major}.{minor}. Exigido: {min_major}.{min_minor}+ (Turing/RTX ou mais nova).",
            gpu = report.name,
            major = report.compute_capability / 10,
            minor = report.compute_capability % 10,
            min_major = MIN_CUDA_13_COMPUTE_CAPABILITY / 10,
            min_minor = MIN_CUDA_13_COMPUTE_CAPABILITY % 10
        )));
    }

    Ok(())
}

fn query_primary_cuda_gpu() -> AppResult<CudaGpuReport> {
    let output = run_nvidia_smi([
        "--id=0",
        "--query-gpu=name,driver_version,compute_cap,memory.total",
        "--format=csv,noheader,nounits",
    ])?;
    parse_nvidia_smi_output(&output)
}

fn run_nvidia_smi<I, S>(arguments: I) -> AppResult<String>
where
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<OsStr>,
{
    let mut failures = Vec::new();

    for executable in nvidia_smi_candidates() {
        let result = Command::new(&executable).args(arguments.clone()).output();
        match result {
            Ok(output) if output.status.success() => {
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
            Ok(output) => failures.push(format!(
                "{}: {}{}",
                display_command(&executable),
                output.status,
                format_process_output(&output.stdout, &output.stderr)
            )),
            Err(error) => failures.push(format!("{}: {error}", display_command(&executable))),
        }
    }

    Err(AppError::SpeechEngineUnavailable(format!(
        "Não consegui consultar a GPU NVIDIA via nvidia-smi. Instale/atualize o driver NVIDIA com suporte CUDA 13.x e reinicie o Windows. Tentativas: {}",
        failures.join(" | ")
    )))
}

fn nvidia_smi_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("nvidia-smi")];

    #[cfg(windows)]
    {
        candidates.push(PathBuf::from(r"C:\Windows\System32\nvidia-smi.exe"));
        candidates.push(PathBuf::from(
            r"C:\Program Files\NVIDIA Corporation\NVSMI\nvidia-smi.exe",
        ));
    }

    candidates
}

fn parse_nvidia_smi_output(output: &str) -> AppResult<CudaGpuReport> {
    let line = output
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| {
            AppError::SpeechEngineUnavailable(
                "nvidia-smi não retornou nenhuma GPU NVIDIA CUDA.".to_string(),
            )
        })?;
    let columns = line.split(',').map(str::trim).collect::<Vec<_>>();

    if columns.len() != 4 {
        return Err(AppError::SpeechEngineUnavailable(format!(
            "Resposta inesperada do nvidia-smi ao consultar a GPU: {line}"
        )));
    }

    let compute_capability = parse_compute_capability(columns[2]).ok_or_else(|| {
        AppError::SpeechEngineUnavailable(format!(
            "Não consegui interpretar a compute capability da GPU NVIDIA ({value}). Atualize o driver NVIDIA e tente de novo.",
            value = columns[2]
        ))
    })?;

    Ok(CudaGpuReport {
        name: columns[0].to_string(),
        driver_version: columns[1].to_string(),
        compute_capability,
        total_memory_mb: parse_memory_mb(columns[3]),
    })
}

fn parse_compute_capability(value: &str) -> Option<u32> {
    let mut parts = value.trim().split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts
        .next()
        .and_then(|part| part.chars().next())
        .and_then(|character| character.to_digit(10))
        .unwrap_or(0);
    Some((major * 10) + minor)
}

fn parse_driver_branch(value: &str) -> Option<u32> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse::<u32>().ok())
}

fn parse_memory_mb(value: &str) -> Option<u64> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse::<u64>().ok())
}

fn format_process_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!(" stdout: {stdout}"),
        (true, false) => format!(" stderr: {stderr}"),
        (false, false) => format!(" stdout: {stdout} stderr: {stderr}"),
    }
}

fn display_command(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(all(test, feature = "ml", feature = "cuda"))]
mod tests {
    use super::*;

    #[test]
    fn parse_nvidia_smi_csv_report() {
        let report = parse_nvidia_smi_output("NVIDIA GeForce RTX 3060, 581.57, 8.6, 12288\n")
            .expect("report");

        assert_eq!(report.name, "NVIDIA GeForce RTX 3060");
        assert_eq!(report.driver_version, "581.57");
        assert_eq!(report.compute_capability, 86);
        assert_eq!(report.total_memory_mb, Some(12_288));
    }

    #[test]
    fn reject_pre_turing_compute_capability() {
        let report = CudaGpuReport {
            name: "NVIDIA GeForce GTX 1080".to_string(),
            driver_version: "581.57".to_string(),
            compute_capability: 61,
            total_memory_mb: Some(8_192),
        };

        let error = validate_compute_capability(&report).expect_err("unsupported GPU");
        assert!(error.to_string().contains("compute capability 6.1"));
    }

    #[test]
    fn reject_pre_cuda_13_driver_branch() {
        let report = CudaGpuReport {
            name: "NVIDIA GeForce RTX 3060".to_string(),
            driver_version: "576.80".to_string(),
            compute_capability: 86,
            total_memory_mb: Some(12_288),
        };

        let error = validate_driver(&report).expect_err("old driver");
        assert!(error.to_string().contains("R580+"));
    }
}
