// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{SecondsFormat, Utc};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

const SUPERVISED_ENV: &str = "NSG_DUB_SUPERVISED";
const MAX_DIALOG_CHARS: usize = 1800;

fn main() {
    if let Some(code) = dublagem_master_tauri_lib::run_whisper_worker_if_requested() {
        std::process::exit(code);
    }

    if std::env::var_os(SUPERVISED_ENV).is_some() {
        dublagem_master_tauri_lib::run();
        return;
    }

    if let Err(error) = supervise_application() {
        let report_path = write_launcher_report(&format!(
            "NSG Gaming Dub launcher failure\nTimestamp: {}\nError: {}\n",
            timestamp(),
            error
        ))
        .ok();
        show_error_dialog(
            "NSG Gaming Dub - erro ao iniciar",
            &format!(
                "Não foi possível iniciar o monitor de falhas.\n\nErro: {}\n{}",
                error,
                report_line(report_path.as_ref())
            ),
        );
        dublagem_master_tauri_lib::run();
    }
}

fn supervise_application() -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("falha ao localizar executável atual: {error}"))?;
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let start_time = Utc::now();
    let mut command = Command::new(&current_exe);
    command.args(&args).env(SUPERVISED_ENV, "1");
    if let Ok(current_dir) = std::env::current_dir() {
        command.current_dir(current_dir);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("falha ao iniciar processo monitorado: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("falha ao aguardar processo monitorado: {error}"))?;

    if status.success() {
        return Ok(());
    }

    let report = format_crash_report(&current_exe, &args, start_time, status);
    let report_path = write_launcher_report(&report).ok();
    show_error_dialog(
        "NSG Gaming Dub - falha fatal",
        &format!(
            "O aplicativo fechou inesperadamente durante o processamento.\n\n{}\n{}",
            exit_status_line(status),
            report_line(report_path.as_ref())
        ),
    );
    Ok(())
}

fn format_crash_report(
    executable: &Path,
    args: &[OsString],
    start_time: chrono::DateTime<Utc>,
    status: ExitStatus,
) -> String {
    let args = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "NSG Gaming Dub supervised process failure\nStarted: {}\nEnded: {}\nExecutable: {}\nArguments: {}\n{}\n\nThe monitored process exited before it could report an application-level error. This usually indicates a native crash in a runtime dependency such as CUDA, Whisper, or a GPU driver.\n",
        start_time.to_rfc3339_opts(SecondsFormat::Millis, true),
        timestamp(),
        executable.display(),
        args,
        exit_status_line(status)
    )
}

fn write_launcher_report(report: &str) -> std::io::Result<PathBuf> {
    let report_dir = dublagem_master_tauri_lib::crash_report_dir();
    std::fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join(format!(
        "supervisor-{}-{}.log",
        Utc::now().format("%Y%m%d-%H%M%S%.3f"),
        std::process::id()
    ));
    std::fs::write(&report_path, report)?;
    Ok(report_path)
}

fn exit_status_line(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!(
            "Código de saída: {} ({})",
            code,
            windows_exit_code_hex(code)
        ),
        None => "Processo finalizado sem código de saída disponível.".to_string(),
    }
}

fn windows_exit_code_hex(code: i32) -> String {
    format!("0x{:08X}", code as u32)
}

fn report_line(report_path: Option<&PathBuf>) -> String {
    report_path
        .map(|path| format!("Relatório: {}", path.display()))
        .unwrap_or_else(|| {
            "Relatório: não foi possível gravar o arquivo de diagnóstico.".to_string()
        })
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(windows)]
fn show_error_dialog(title: &str, message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TASKMODAL, MB_TOPMOST,
    };

    let title = wide_null(title);
    let message = wide_null(&truncate_dialog_message(message));
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST | MB_TASKMODAL,
        );
    }
}

#[cfg(not(windows))]
fn show_error_dialog(title: &str, message: &str) {
    eprintln!("{title}\n{message}");
}

fn truncate_dialog_message(message: &str) -> String {
    if message.chars().count() <= MAX_DIALOG_CHARS {
        return message.to_string();
    }

    let mut truncated = message.chars().take(MAX_DIALOG_CHARS).collect::<String>();
    truncated.push_str("\n...");
    truncated
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
