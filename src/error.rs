use std::io::Error;
use std::process::ExitStatus;

#[derive(Debug)]
pub enum AppError {
    CommandFailed(ExitStatus),
    IoError(Error),
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
