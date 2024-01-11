use crate::error::AppError;
use std::io::{self, BufRead, BufReader};
use std::process::Child;

pub fn stream_output(output: &mut Child) -> Result<String, AppError> {
    let stdout = output
        .stdout
        .take()
        .ok_or(AppError::IoError(io::Error::from(
            io::ErrorKind::BrokenPipe,
        )))?;
    let stdout_reader = BufReader::new(stdout);
    let mut stdout_lines = stdout_reader.lines().peekable();

    let mut result = String::new();
    while let Some(Ok(line)) = stdout_lines.next() {
        if stdout_lines.peek().is_some() {
            println!("{:?}", &line);
        }
        result += &line;
        result.push('\n');
    }

    output.wait().map_err(|e| AppError::from(e))?;

    return Ok(result);
}
