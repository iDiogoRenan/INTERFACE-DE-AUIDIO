use chrono::{SecondsFormat, Utc};
use std::{backtrace::Backtrace, panic::PanicHookInfo, path::PathBuf, sync::OnceLock};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
const MAX_DIALOG_CHARS: usize = 1800;

pub fn install_panic_reporter() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let payload = panic_payload(panic_info);
        let location = panic_location(panic_info);
        let report = format_panic_report(&payload, &location);
        let report_path = write_panic_report(&report).ok();

        if let Some(app) = APP_HANDLE.get() {
            show_panic_dialog(app, &payload, &location, report_path.as_ref());
        }

        default_hook(panic_info);
    }));
}

pub fn register_app_handle(app: &tauri::App) {
    let _ = APP_HANDLE.set(app.handle().clone());
}

fn format_panic_report(payload: &str, location: &str) -> String {
    format!(
        "NSG Gaming Dub fatal panic\nTimestamp: {}\nProcess ID: {}\nLocation: {}\nMessage: {}\n\nBacktrace:\n{}\n",
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        std::process::id(),
        location,
        payload,
        Backtrace::force_capture()
    )
}

fn write_panic_report(report: &str) -> std::io::Result<PathBuf> {
    let report_dir = crash_report_dir();
    std::fs::create_dir_all(&report_dir)?;
    let file_name = format!(
        "panic-{}-{}.log",
        Utc::now().format("%Y%m%d-%H%M%S%.3f"),
        std::process::id()
    );
    let report_path = report_dir.join(file_name);
    std::fs::write(&report_path, report)?;
    Ok(report_path)
}

pub fn crash_report_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("crash-reports")))
        .unwrap_or_else(|| PathBuf::from("crash-reports"))
}

fn show_panic_dialog(
    app: &AppHandle,
    payload: &str,
    location: &str,
    report_path: Option<&PathBuf>,
) {
    let report_line = report_path
        .map(|path| format!("Relatório: {}", path.display()))
        .unwrap_or_else(|| {
            "Relatório: não foi possível gravar o arquivo de diagnóstico.".to_string()
        });
    let message = truncate_dialog_message(&format!(
        "O aplicativo encontrou um erro fatal e precisa ser encerrado.\n\nErro: {}\nLocal: {}\n{}",
        payload, location, report_line
    ));

    let _ = app
        .dialog()
        .message(message)
        .kind(MessageDialogKind::Error)
        .title("NSG Gaming Dub - erro fatal")
        .blocking_show();
}

fn panic_payload(panic_info: &PanicHookInfo<'_>) -> String {
    if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = panic_info.payload().downcast_ref::<String>() {
        return message.clone();
    }
    "panic sem mensagem textual".to_string()
}

fn panic_location(panic_info: &PanicHookInfo<'_>) -> String {
    panic_info
        .location()
        .map(|location| {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        })
        .unwrap_or_else(|| "local desconhecido".to_string())
}

fn truncate_dialog_message(message: &str) -> String {
    if message.chars().count() <= MAX_DIALOG_CHARS {
        return message.to_string();
    }

    let mut truncated = message.chars().take(MAX_DIALOG_CHARS).collect::<String>();
    truncated.push_str("\n...");
    truncated
}
