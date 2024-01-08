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
    let stdout_lines = stdout_reader.lines();

    let mut result = String::new();
    for line in stdout_lines {
        let l = line?;
        println!("{:?}", &l);
        result.push_str(&l);
        result.push('\n');
    }

    output.wait().map_err(|e| AppError::from(e))?;

    return Ok(result);
}
