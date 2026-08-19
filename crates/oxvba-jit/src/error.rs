//! Public JIT error and outcome types.

use super::*;

/// Final `Err` state surfaced by the JIT backend without depending on `oxvba-host`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JitFinalErr {
    pub number: i32,
    pub source: String,
    pub description: String,
    pub last_dll_error: i32,
}

/// Observable result of a JIT run.
#[derive(Debug, Clone)]
pub struct JitOutcome {
    pub values: Vec<Variant>,
    pub err: JitFinalErr,
    pub raised: bool,
}

#[derive(Debug, Error)]
pub enum JitError {
    #[error("jit: unsupported: {0}")]
    Unsupported(String),
    #[error("jit compile: {0}")]
    Compile(String),
    #[error("jit runtime: {0}")]
    Runtime(String),
}

impl JitError {
    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::Unsupported(what.into())
    }

    pub fn unsupported_message(&self) -> Option<&str> {
        match self {
            Self::Unsupported(what) => Some(what.as_str()),
            _ => None,
        }
    }
}
