use crate::error::AppError;
use crate::command::Command;

pub struct App<'a>{
    commands: Vec<&'a Command>,
}

impl<'a> App<'a> {
    pub fn new(commands: Vec<&'a Command>) -> Self {
        Self { commands }
    }

    pub fn execute(&self) -> Result<(), AppError> {
        for command in &self.commands {
            println!("Executing command: {:?}", command);
            command.execute()?;
            // wait 1 second
            // std::thread::sleep(std::time::Duration::from_secs(1));
        }

        Ok(())
    }
}
