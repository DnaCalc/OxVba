use std::fmt;

use oxvba_host::{DirectHostBreakpointId, DirectHostStackFrameId, DirectHostWatchId};

/// Errors returned while attaching a debug session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugAttachError {
    Compile {
        message: String,
    },
    Prepare {
        message: String,
    },
    WorkerFailed {
        stage: &'static str,
        message: String,
    },
    Unsupported(&'static str),
}

impl fmt::Display for DebugAttachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile { message } => write!(f, "debug attach compile failed: {message}"),
            Self::Prepare { message } => write!(f, "debug attach prepare failed: {message}"),
            Self::WorkerFailed { stage, message } => {
                write!(f, "debug worker failed during {stage}: {message}")
            }
            Self::Unsupported(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for DebugAttachError {}

/// Typed command/lifecycle errors for a debug session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugError {
    NotPaused,
    UnknownBreakpoint(DirectHostBreakpointId),
    UnknownWatch(DirectHostWatchId),
    UnknownFrame(DirectHostStackFrameId),
    Evaluation {
        expression: String,
        message: String,
    },
    Completed,
    UnsupportedCommand(&'static str),
    OutstandingHandles {
        count: usize,
    },
    SessionAlreadyDetached,
    WorkerFailed {
        stage: &'static str,
        message: String,
    },
    Internal(String),
}

impl fmt::Display for DebugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPaused => f.write_str("debug session is not paused"),
            Self::UnknownBreakpoint(id) => write!(f, "unknown breakpoint: {id}"),
            Self::UnknownWatch(id) => write!(f, "unknown watch: {id}"),
            Self::UnknownFrame(id) => write!(f, "unknown frame: {id}"),
            Self::Evaluation {
                expression,
                message,
            } => write!(f, "evaluation failed for {expression:?}: {message}"),
            Self::Completed => f.write_str("debug session has completed"),
            Self::UnsupportedCommand(command) => write!(f, "unsupported debug command: {command}"),
            Self::OutstandingHandles { count } => {
                write!(f, "cannot detach while {count} handle clones remain")
            }
            Self::SessionAlreadyDetached => f.write_str("debug session is already detached"),
            Self::WorkerFailed { stage, message } => {
                write!(f, "debug worker failed during {stage}: {message}")
            }
            Self::Internal(message) => write!(f, "internal debug error: {message}"),
        }
    }
}

impl std::error::Error for DebugError {}
