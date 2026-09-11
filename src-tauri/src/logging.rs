use std::{
    collections::HashMap,
    panic::Location,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::core::{install_command_error_observer, ErrorDiagnostic};

// The non-blocking writer guard must outlive the subscriber; dropping it would
// stop the background writer and lose buffered log lines. Keeping it in a
// process-lifetime static pins it for the whole app run.
static LOG_GUARD: OnceLock<Mutex<Option<WorkerGuard>>> = OnceLock::new();

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
    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("nextmail.log")
        .max_log_files(14)
        .build(&log_dir);
    let output: Box<dyn std::io::Write + Send> = match file_appender {
        Ok(appender) => Box::new(appender),
        Err(_) => {
            eprintln!("nextmail: file logging unavailable; using stderr");
            Box::new(std::io::stderr())
        }
    };
    let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(1024)
        .finish(output);
    if LOG_GUARD.set(Mutex::new(Some(guard))).is_err() {
        eprintln!("nextmail: logging worker guard was already initialized");
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if let Err(error) = tracing_subscriber::registry()
        .with(filter)
        // Dependency TRACE/DEBUG output may include entire protocol messages.
        .with(tracing_subscriber::filter::filter_fn(|metadata| {
            metadata.target().starts_with("nextmail")
        }))
        .with(fmt::layer().with_writer(writer).with_ansi(false))
        .try_init()
    {
        eprintln!("nextmail: failed to initialize tracing subscriber: {error}");
    }

    if !install_command_error_observer(report_command_error) {
        tracing::warn!("command error observer was already initialized");
    }
    install_panic_hook();
    tracing::info!(retained_files = 14, "logging initialized");
}

pub(crate) fn shutdown() {
    // Explicit shutdown flushes the bounded queue; statics are not dropped at exit.
    if let Some(guard) = LOG_GUARD.get() {
        if let Ok(mut guard) = guard.lock() {
            drop(guard.take());
        }
    }
}

#[derive(Default)]
pub(crate) struct ErrorRateLimit {
    entries: HashMap<String, (Instant, u64)>,
}

impl ErrorRateLimit {
    pub(crate) fn record(&mut self, key: String, now: Instant) -> Option<u64> {
        if let Some((last, suppressed)) = self.entries.get_mut(&key) {
            if now.duration_since(*last) < Duration::from_secs(60) {
                *suppressed = suppressed.saturating_add(1);
                return None;
            }
            let count = *suppressed;
            *last = now;
            *suppressed = 0;
            return Some(count);
        }
        if self.entries.len() >= 256 {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(key, (now, 0));
        Some(0)
    }
}

fn report_command_error(
    code: &str,
    retryable: bool,
    diagnostic: Option<&ErrorDiagnostic>,
    location: &'static Location<'static>,
) {
    static LIMIT: OnceLock<Mutex<ErrorRateLimit>> = OnceLock::new();
    let key = format!(
        "{code}:{}:{}:{}",
        location.file(),
        location.line(),
        diagnostic.map_or("unspecified", |d| d.cause)
    );
    let Some(suppressed) = LIMIT
        .get_or_init(Mutex::default)
        .lock()
        .ok()
        .and_then(|mut limit| limit.record(key, Instant::now()))
    else {
        return;
    };
    tracing::warn!(
        %code,
        retryable,
        cause = diagnostic.map(|d| d.cause),
        error_type = diagnostic.map(|d| d.error_type),
        os_code = diagnostic.and_then(|d| d.os_code),
        database_code = diagnostic.and_then(|d| d.database_code),
        source.file = location.file(),
        source.line = location.line(),
        suppressed,
        "backend operation failed"
    );
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "<unknown>".to_owned());
        // Panic payloads and the default hook can expose message/credential data.
        tracing::error!(location = %location, "panic (payload redacted)");
    }));
}

fn frontend_descriptor(message: &str) -> Option<(String, &'static str, bool)> {
    static CATALOG: OnceLock<serde_json::Value> = OnceLock::new();
    if message.len() > 1024 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(message).ok()?;
    let code = value.get("code")?.as_str()?;
    let catalog = CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/locales/en-US/common.json"))
            .expect("bundled error catalog")
    });
    let code = if catalog["errors"].get(code).is_some() {
        code
    } else {
        "common.unexpected_error"
    };
    let context = match value.get("context").and_then(|value| value.as_str()) {
        Some("ipc") => "ipc",
        Some("uncaught") => "uncaught",
        Some("rejection") => "rejection",
        _ => "caught",
    };
    Some((
        code.to_owned(),
        context,
        value
            .get("retryable")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    ))
}

pub(crate) fn report_frontend_error(message: &str) {
    static LIMIT: OnceLock<Mutex<ErrorRateLimit>> = OnceLock::new();
    let Some((code, context, retryable)) = frontend_descriptor(message) else {
        return;
    };
    let Some(suppressed) = LIMIT
        .get_or_init(Mutex::default)
        .lock()
        .ok()
        .and_then(|mut limit| limit.record(format!("{context}:{code}"), Instant::now()))
    else {
        return;
    };
    tracing::warn!(%code, context, retryable, suppressed, "frontend operation failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_errors_are_counted_and_bookkeeping_is_bounded() {
        let mut limit = ErrorRateLimit::default();
        let now = Instant::now();
        assert_eq!(limit.record("failure".into(), now), Some(0));
        for _ in 0..10_000 {
            assert_eq!(limit.record("failure".into(), now), None);
        }
        assert_eq!(
            limit.record("failure".into(), now + Duration::from_secs(60)),
            Some(10_000)
        );
        for index in 0..10_000 {
            limit.record(index.to_string(), now);
        }
        assert!(limit.entries.len() <= 256);
    }
    #[test]
    fn frontend_payload_cannot_write_arbitrary_text_to_logs() {
        assert!(frontend_descriptor("password token /private/path").is_none());
        let descriptor = frontend_descriptor(r#"{"code":"secret-token","context":"secret-path","params":{"password":"secret"},"stack":"secret"}"#).unwrap();
        assert_eq!(
            descriptor,
            ("common.unexpected_error".into(), "caught", false)
        );
    }
}
