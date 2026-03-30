use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Windows(windows::core::Error),
    WindowsService(windows_service::Error),
    Registry {
        operation: &'static str,
        path: String,
        code: u32,
    },
    InvalidParameterValue {
        parameter: String,
        value: String,
    },
    UnknownParameter(String),
    InvalidServiceState(u32),
    Message(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Windows(error) => write!(f, "{error}"),
            Self::WindowsService(error) => write!(f, "{error}"),
            Self::Registry {
                operation,
                path,
                code,
            } => write!(f, "Registry {operation} failed for '{path}' (code {code})"),
            Self::InvalidParameterValue { parameter, value } => {
                write!(f, "Invalid value '{value}' for parameter '{parameter}'")
            }
            Self::UnknownParameter(parameter) => write!(f, "Unknown parameter: {parameter}"),
            Self::InvalidServiceState(state) => write!(f, "Invalid Windows service state: {state}"),
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<windows::core::Error> for AppError {
    fn from(value: windows::core::Error) -> Self {
        Self::Windows(value)
    }
}

impl From<windows_service::Error> for AppError {
    fn from(value: windows_service::Error) -> Self {
        Self::WindowsService(value)
    }
}

impl From<std::num::ParseIntError> for AppError {
    fn from(value: std::num::ParseIntError) -> Self {
        Self::Message(value.to_string())
    }
}

impl From<ctrlc::Error> for AppError {
    fn from(value: ctrlc::Error) -> Self {
        Self::Message(value.to_string())
    }
}
