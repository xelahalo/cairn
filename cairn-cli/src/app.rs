use crate::command::MutCommand;
use crate::error::AppError;

pub struct App<'a> {
    commands: Vec<Box<dyn MutCommand + 'a>>,
}

impl<'a> App<'a> {
    pub fn new(commands: Vec<Box<dyn MutCommand + 'a>>) -> Self {
        Self { commands }
    }

    pub fn execute(&mut self) -> Result<(), AppError> {
        for command in self.commands.iter_mut() {
            command.execute()?;
        }

        Ok(())
    }
}
