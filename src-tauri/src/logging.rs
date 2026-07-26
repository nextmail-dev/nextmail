use std::sync::OnceLock;

use tauri::{AppHandle, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::core::install_command_error_observer;

// The non-blocking writer guard must outlive the subscriber; dropping it would
// stop the background writer and lose buffered log lines. Keeping it in a
// process-lifetime static pins it for the whole app run.
static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Initialises process-wide structured logging to a rolling file under the
/// app's local data directory, and installs a panic hook that records panics.
///
/// Logs are written to `<app_local_data_dir>/logs/nextmail.log.YYYY-MM-DD`.
/// The level is `info` by default and can be overridden with `RUST_LOG`.
pub fn init(app: &AppHandle) {
    let log_dir = app
        .path()
        .app_local_data_dir()
        .map(|dir| dir.join("logs"))
        .unwrap_or_else(|_| std::env::temp_dir().join("nextmail").join("logs"));
    if let Err(error) = std::fs::create_dir_all(&log_dir) {
        eprintln!("nextmail: failed to create log directory {log_dir:?}: {error}");
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "nextmail.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    if LOG_GUARD.set(guard).is_err() {
        eprintln!("nextmail: logging worker guard was already initialized");
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if let Err(error) = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(writer).with_ansi(false))
        .try_init()
    {
        eprintln!("nextmail: failed to initialize tracing subscriber: {error}");
    }

    if !install_command_error_observer(report_command_error) {
        tracing::warn!("command error observer was already initialized");
    }
    install_panic_hook();
    tracing::info!(?log_dir, "logging initialized");
}

fn report_command_error(code: &str, retryable: bool, file: &'static str, line: u32, column: u32) {
    tracing::warn!(
        %code,
        retryable,
        source.file = file,
        source.line = line,
        source.column = column,
        "stable backend error created"
    );
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "<unknown>".to_owned());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("<non-string panic payload>");
        tracing::error!(location = %location, panic = %message, "panic");
        previous(info);
    }));
}
