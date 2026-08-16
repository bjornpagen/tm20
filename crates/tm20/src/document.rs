//! A document is a list of commands. Nothing else.

use crate::command::Command;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document(pub Vec<Command>);

impl Document {
    pub fn new(commands: impl Into<Vec<Command>>) -> Self {
        Document(commands.into())
    }

    pub fn commands(&self) -> &[Command] {
        &self.0
    }
}

impl From<Vec<Command>> for Document {
    fn from(commands: Vec<Command>) -> Self {
        Document(commands)
    }
}

impl FromIterator<Command> for Document {
    fn from_iter<T: IntoIterator<Item = Command>>(iter: T) -> Self {
        Document(iter.into_iter().collect())
    }
}
