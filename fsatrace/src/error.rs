use std::io::Error;
use std::process::ExitStatus;
use std::time::SystemTimeError;

#[derive(Debug)]
pub enum AppError {
    CommandFailed(ExitStatus),
    IoError(Error),
    SystemTimeError(SystemTimeError),
    EnvVarError(Error),
    Unknown,
}

impl From<ExitStatus> for AppError {
    fn from(status: ExitStatus) -> Self {
        AppError::CommandFailed(status)
    }
}

impl From<Error> for AppError {
    fn from(err: Error) -> Self {
        AppError::IoError(err)
    }
}

impl From<SystemTimeError> for AppError {
    fn from(err: SystemTimeError) -> Self {
        AppError::SystemTimeError(err)
    }
}
