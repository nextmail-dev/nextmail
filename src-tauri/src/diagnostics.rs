//! Error classification at infrastructure boundaries. Never format external errors.
use crate::core::{CommandError, ErrorDiagnostic};
use std::{
    any::{type_name, Any},
    io,
};

#[track_caller]
pub(crate) fn lock_error<T>(code: &str, _: &std::sync::PoisonError<T>) -> CommandError {
    CommandError::diagnosed(
        code,
        false,
        ErrorDiagnostic {
            cause: "task_panicked",
            error_type: "PoisonError",
            ..Default::default()
        },
    )
}

#[track_caller]
pub(crate) fn command_error<E: Any>(code: &str, retryable: bool, error: &E) -> CommandError {
    command_error_at(code, retryable, error, std::panic::Location::caller())
}

pub(crate) fn command_error_at<E: Any>(
    code: &str,
    retryable: bool,
    error: &E,
    location: &'static std::panic::Location<'static>,
) -> CommandError {
    let diagnostic = classify(error);
    // Preserve the caller's retry policy: classification must never make a
    // partially completed SMTP or filesystem operation eligible for replay.
    CommandError::diagnosed_at(code, retryable, diagnostic, location)
}

pub(crate) fn classify<E: Any>(error: &E) -> ErrorDiagnostic {
    let value = error as &dyn Any;
    let mut result = ErrorDiagnostic {
        cause: "unknown",
        error_type: type_name::<E>(),
        ..Default::default()
    };
    if let Some(error) = value.downcast_ref::<io::Error>() {
        classify_io(error, &mut result);
    } else if let Some(error) = value.downcast_ref::<tempfile::PersistError>() {
        classify_io(&error.error, &mut result);
    } else if let Some(error) = value.downcast_ref::<serde_json::Error>() {
        result.cause = match error.classify() {
            serde_json::error::Category::Io => "io",
            _ => "invalid_data",
        };
    } else if let Some(error) = value.downcast_ref::<sqlx::Error>() {
        match error {
            sqlx::Error::Database(database) => {
                result.database_code = database.code().and_then(|code| code.parse().ok());
                result.cause = match result.database_code.map(|code| code & 0xff) {
                    Some(5 | 6) => "database_busy",
                    Some(8) => "read_only",
                    Some(13) => "disk_full",
                    Some(11 | 26) => "invalid_data",
                    Some(19) => "constraint",
                    _ => "database",
                };
            }
            sqlx::Error::Io(error) => classify_io(error, &mut result),
            sqlx::Error::PoolTimedOut => result.cause = "database_busy",
            sqlx::Error::RowNotFound => result.cause = "not_found",
            _ => result.cause = "database",
        }
    } else if let Some(error) = value.downcast_ref::<async_imap::error::Error>() {
        match error {
            async_imap::error::Error::Io(error) => classify_io(error, &mut result),
            async_imap::error::Error::No(_) | async_imap::error::Error::Bad(_) => {
                result.cause = "server_rejected"
            }
            async_imap::error::Error::Parse(_) => result.cause = "protocol",
            async_imap::error::Error::ConnectionLost => result.cause = "connection_lost",
            _ => result.cause = "protocol",
        }
    } else if let Some(error) = value.downcast_ref::<reqwest::Error>() {
        result.cause = if error.is_timeout() {
            "timeout"
        } else if error.is_connect() {
            "connection_lost"
        } else {
            "network"
        };
    } else if let Some(error) = value.downcast_ref::<lettre::transport::smtp::Error>() {
        result.cause = if error.is_timeout() {
            "timeout"
        } else if error.is_client() {
            "invalid_data"
        } else {
            "server_rejected"
        };
    } else if let Some(error) = value.downcast_ref::<keyring::Error>() {
        result.cause = match error {
            keyring::Error::NoEntry => "credential_missing",
            keyring::Error::NoStorageAccess(_) => "permission_denied",
            _ => "credential_store",
        };
    } else if let Some(error) = value.downcast_ref::<tokio::task::JoinError>() {
        result.cause = if error.is_panic() {
            "task_panicked"
        } else {
            "task_cancelled"
        };
    }
    result
}

fn classify_io(error: &io::Error, result: &mut ErrorDiagnostic) {
    result.os_code = error.raw_os_error();
    result.cause = if cfg!(windows) && matches!(result.os_code, Some(32 | 33 | 1224)) {
        "file_busy"
    } else {
        match error.kind() {
            io::ErrorKind::PermissionDenied => "permission_denied",
            io::ErrorKind::NotFound => "not_found",
            io::ErrorKind::StorageFull => "disk_full",
            io::ErrorKind::ReadOnlyFilesystem => "read_only",
            io::ErrorKind::TimedOut => "timeout",
            io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::UnexpectedEof => "connection_lost",
            io::ErrorKind::ConnectionRefused => "connection_refused",
            io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => "invalid_data",
            _ => "io",
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn file_mapping_conflicts_keep_the_os_code_without_changing_retry_policy() {
        let error = io::Error::from_raw_os_error(1224);
        assert_eq!(classify(&error).cause, "file_busy");
        assert_eq!(classify(&error).os_code, Some(1224));
        assert!(!command_error("storage.accounts_write_failed", false, &error).retryable);
    }
    #[test]
    fn redacts_external_errors_but_keeps_actionable_classification() {
        let secret = "password token mailbox message /private/path";
        for (error, reason) in [
            (
                io::Error::new(io::ErrorKind::PermissionDenied, secret),
                "permission_denied",
            ),
            (
                io::Error::new(io::ErrorKind::StorageFull, secret),
                "disk_full",
            ),
        ] {
            let public = command_error("storage.accounts_write_failed", false, &error);
            assert_eq!(public.params["reason"], reason);
            assert!(!format!("{:?}", classify(&error)).contains(secret));
            assert!(!serde_json::to_string(&public).unwrap().contains(secret));
        }
        let error = async_imap::error::Error::No(secret.to_owned());
        assert_eq!(classify(&error).cause, "server_rejected");
        assert!(!format!("{:?}", classify(&error)).contains(secret));
    }
}
