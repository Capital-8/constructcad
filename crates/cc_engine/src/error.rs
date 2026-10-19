use serde::Serialize;
use std::fmt;

/// What kind of failure a command met.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No command has this name.
    UnknownCommand,
    /// The arguments do not match the command's schema.
    InvalidArguments,
    /// A referenced element, type, level, view or material does not exist.
    NotFound,
    /// The edit would leave the model invalid, or is not allowed now.
    Rejected,
    /// Reading or writing a file failed.
    Io,
    /// An internal error (a bug); the session is kept as it was.
    Internal,
}

/// A command failure: a code for programs and a message for people.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineError {
    pub code: ErrorCode,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, EngineError>;

impl EngineError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArguments, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn rejected(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Rejected, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Io, message)
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EngineError {}

impl From<cc_core::wall_ops::EditError> for EngineError {
    fn from(e: cc_core::wall_ops::EditError) -> Self {
        use cc_core::wall_ops::EditError as E;
        Self::rejected(match e {
            E::NotLinear => "the element has no location line",
            E::Parallel => "the lines are parallel",
            E::Degenerate => "the result would have no length",
        })
    }
}
