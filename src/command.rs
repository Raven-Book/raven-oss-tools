use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use crate::parser::Arguments;

pub type CommandHandler = Box<dyn Fn(Arguments) -> Pin<Box<dyn Future<Output=Result<(), String>>>>>;

pub(crate) struct CommandRegistry {
    commands: HashMap<String, CommandHandler>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new()
        }
    }

    pub fn register(&mut self, command_name: impl Into<String>, handler: CommandHandler) {
        self.commands.insert(command_name.into(), handler);
    }

    pub async fn execute(&mut self, arguments: Arguments) -> Result<(), String> {
        let main_command = arguments.main_command.clone();
        if main_command.is_none() {
            println!("缺少主指令！");
            return Ok(())
        }

        match self.commands.get(&main_command.unwrap()) {
            Some(handler) => handler(arguments).await,
            None => {
                println!("未找到命令： {:?}", arguments.main_command.unwrap());
                Ok(())
            }
        }
    }
}