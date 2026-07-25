use std::sync::OnceLock;

use tauri::{AppHandle, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "nextmail.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(writer).with_ansi(false))
        .try_init();

    install_panic_hook();
    tracing::info!(?log_dir, "logging initialized");
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
