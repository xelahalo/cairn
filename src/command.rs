use crate::error::AppError;
use crate::util::stream_output;

#[derive(Debug)]
pub struct Command {
    executable: String,
    args: Vec<String>,
}

impl Command {
    pub fn new(executable: &str, args: Vec<&str>) -> Self {
        Self {
            executable: executable.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn execute(&self) -> Result<(), AppError> {
        let mut child = std::process::Command::new(&self.executable)
            .args(&self.args)
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        stream_output(&mut child)
    }
}
