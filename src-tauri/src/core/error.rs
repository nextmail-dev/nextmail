use std::{collections::BTreeMap, fmt, panic::Location, sync::OnceLock};

use serde::Serialize;

pub type CommandResult<T> = Result<T, CommandError>;

pub type CommandErrorObserver = fn(
    code: &str,
    retryable: bool,
    diagnostic: Option<&ErrorDiagnostic>,
    location: &'static Location<'static>,
);

/// Only classifications and numeric codes, never an external error's text.
#[derive(Clone, Debug, Default)]
pub struct ErrorDiagnostic {
    pub cause: &'static str,
    pub error_type: &'static str,
    pub os_code: Option<i32>,
    pub database_code: Option<i32>,
}

static COMMAND_ERROR_OBSERVER: OnceLock<CommandErrorObserver> = OnceLock::new();

/// Installs the process-level observer for stable backend errors.
///
/// The observer receives only the stable code, retryability, safe classification
/// and Rust call site. `CommandError::params` can contain user-entered values and
/// is therefore never forwarded to diagnostics.
pub fn install_command_error_observer(observer: CommandErrorObserver) -> bool {
    COMMAND_ERROR_OBSERVER.set(observer).is_ok()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub params: BTreeMap<String, String>,
    pub retryable: bool,
}

impl CommandError {
    #[track_caller]
    pub fn new(code: impl Into<String>) -> Self {
        Self::build(code.into(), false, None, Location::caller())
    }

    #[track_caller]
    pub fn retryable(code: impl Into<String>) -> Self {
        Self::build(code.into(), true, None, Location::caller())
    }

    #[track_caller]
    pub fn diagnosed(
        code: impl Into<String>,
        retryable: bool,
        diagnostic: ErrorDiagnostic,
    ) -> Self {
        Self::build(code.into(), retryable, Some(diagnostic), Location::caller())
    }

    pub fn diagnosed_at(
        code: impl Into<String>,
        retryable: bool,
        diagnostic: ErrorDiagnostic,
        location: &'static Location<'static>,
    ) -> Self {
        Self::build(code.into(), retryable, Some(diagnostic), location)
    }

    fn build(
        code: String,
        retryable: bool,
        diagnostic: Option<ErrorDiagnostic>,
        location: &'static Location<'static>,
    ) -> Self {
        let mut error = Self {
            code,
            params: BTreeMap::new(),
            retryable,
        };
        if let Some(observer) = COMMAND_ERROR_OBSERVER.get() {
            observer(&error.code, error.retryable, diagnostic.as_ref(), location);
        }
        if let Some(diagnostic) = diagnostic {
            error
                .params
                .insert("reason".to_owned(), diagnostic.cause.to_owned());
        }
        error
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }
}

impl fmt::Debug for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandError")
            .field("code", &self.code)
            .field("retryable", &self.retryable)
            .finish_non_exhaustive()
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

    fn count_observation(
        _: &str,
        _: bool,
        _: Option<&super::ErrorDiagnostic>,
        _: &'static std::panic::Location<'static>,
    ) {
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
        assert!(!format!("{error:?}").contains("must-not-reach-observer"));
    }
}
