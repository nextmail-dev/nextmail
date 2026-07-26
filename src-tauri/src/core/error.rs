use std::{collections::BTreeMap, fmt, panic::Location, sync::OnceLock};

use serde::Serialize;

pub type CommandResult<T> = Result<T, CommandError>;

pub type CommandErrorObserver =
    fn(code: &str, retryable: bool, file: &'static str, line: u32, column: u32);

static COMMAND_ERROR_OBSERVER: OnceLock<CommandErrorObserver> = OnceLock::new();

/// Installs the process-level observer for stable backend errors.
///
/// The observer deliberately receives only the stable code, retryability and
/// Rust call site. `CommandError::params` can contain user-entered values and is
/// therefore never forwarded to diagnostics.
pub fn install_command_error_observer(observer: CommandErrorObserver) -> bool {
    COMMAND_ERROR_OBSERVER.set(observer).is_ok()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub params: BTreeMap<String, String>,
    pub retryable: bool,
}

impl CommandError {
    #[track_caller]
    pub fn new(code: impl Into<String>) -> Self {
        Self::build(code.into(), false, Location::caller())
    }

    #[track_caller]
    pub fn retryable(code: impl Into<String>) -> Self {
        Self::build(code.into(), true, Location::caller())
    }

    fn build(code: String, retryable: bool, location: &'static Location<'static>) -> Self {
        let error = Self {
            code,
            params: BTreeMap::new(),
            retryable,
        };
        if let Some(observer) = COMMAND_ERROR_OBSERVER.get() {
            observer(
                &error.code,
                error.retryable,
                location.file(),
                location.line(),
                location.column(),
            );
        }
        error
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.code)
    }
}

impl std::error::Error for CommandError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{install_command_error_observer, CommandError};

    static OBSERVED: AtomicUsize = AtomicUsize::new(0);

    fn count_observation(_: &str, _: bool, _: &'static str, _: u32, _: u32) {
        OBSERVED.fetch_add(1, Ordering::Relaxed);
    }

    #[test]
    fn observes_error_construction_without_exposing_params() {
        let _ = install_command_error_observer(count_observation);
        let before = OBSERVED.load(Ordering::Relaxed);
        let error =
            CommandError::retryable("sync.failed").with_param("secret", "must-not-reach-observer");
        assert_eq!(error.code, "sync.failed");
        assert!(error.retryable);
        assert!(OBSERVED.load(Ordering::Relaxed) > before);
    }
}
