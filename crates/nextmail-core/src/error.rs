use std::{collections::BTreeMap, fmt};

use serde::Serialize;

pub type CommandResult<T> = Result<T, CommandError>;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub params: BTreeMap<String, String>,
    pub retryable: bool,
}

impl CommandError {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            params: BTreeMap::new(),
            retryable: false,
        }
    }

    pub fn retryable(code: impl Into<String>) -> Self {
        Self {
            retryable: true,
            ..Self::new(code)
        }
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
