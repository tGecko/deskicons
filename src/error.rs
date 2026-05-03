use std::io;

use windows::Win32::Foundation::GetLastError;
use windows::core::{BOOL, Error as WinError};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    Windows(WinError),
    Message(String),
}

impl AppError {
    pub fn message(value: impl Into<String>) -> Self {
        Self::Message(value.into())
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Windows(err) => f.write_str(&err.message()),
            Self::Message(value) => f.write_str(value),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Windows(err) => Some(err),
            Self::Message(_) => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<WinError> for AppError {
    fn from(value: WinError) -> Self {
        Self::Windows(value)
    }
}

pub fn last_error(what: &str) -> AppError {
    AppError::message(format!(
        "{what}: {}",
        unsafe { GetLastError() }.to_hresult().message()
    ))
}

pub fn check_bool(ok: BOOL, what: &str) -> Result<()> {
    if ok.as_bool() {
        Ok(())
    } else {
        Err(last_error(what))
    }
}
