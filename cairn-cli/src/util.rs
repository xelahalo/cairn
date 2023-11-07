use crate::error::AppError;
use std::io::{self, BufRead, BufReader};
use std::process::Child;

pub fn stream_output(output: &mut Child) -> Result<(), AppError> {
    let stdout = output.stdout.take().ok_or(AppError::IoError(io::Error::from(
        io::ErrorKind::BrokenPipe,
    )))?;
    let stdout_reader = BufReader::new(stdout);
    let stdout_lines = stdout_reader.lines();

    for line in stdout_lines {
        println!("Read: {:?}", line);
    }

    output.wait().map_err(|e| AppError::from(e))?;

    Ok(())
}

// pub fn get_env_vars_or_panic(keys: Vec<&str>) -> Vec<String> {
//     keys.iter()
//         .map(|key| {
//             std::env::var(key).unwrap_or_else(|_| {
//                 panic!(
//                     "Environment variable {} not set",
//                     key
//                 )
//             })
//         })
//         .collect()
// }
