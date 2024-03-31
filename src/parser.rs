use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub struct Arguments {
    pub(crate) flags: Vec<String>,
    pub(crate) positional: Vec<String>,
    pub(crate) main_command: Option<String>,
    pub(crate) optional: HashMap<String, String>,
}

pub struct CommandParser;

impl CommandParser {
    pub fn from_strings<I: IntoIterator<Item=impl Into<String>>>(args: I) -> Arguments {
        let mut flags: Vec<String> = Vec::new();
        let mut positional: Vec<String> = Vec::new();
        let mut optional: HashMap<String, String> = HashMap::new();
        let mut main_command: Option<String> = None;

        let mut iter = args.into_iter().skip(1);

        let mut buffer: Option<String> = None;

        while let Some(arg) = buffer.take().or_else(|| iter.next().map(|arg| arg.into())) {
            if arg.starts_with("--") {
                if arg.len() < 3 {
                    continue;
                }

                let flag = &arg[2..];
                flags.push(flag.into());
            } else if arg.starts_with('-') {
                if arg.len() < 2 {
                    continue;
                }

                if let Some(next_arg) = buffer.take().or_else(|| iter.next().map(|arg| arg.into())) {
                    if next_arg.starts_with('-') && next_arg.len() > 1 {
                        buffer = Some(next_arg);
                        flags.push(arg[1..].into());
                    } else {
                        optional.insert(arg[1..].into(), next_arg);
                    }
                }
            } else if arg.contains('=') && !(arg.starts_with('=') || arg.ends_with('=')) {
                let mut parts = arg.splitn(2, '=');
                optional.insert(parts.next().unwrap().into(),
                                parts.next().unwrap().into());
            } else if main_command.is_none() {
                main_command = Some(arg);
            } else {
                positional.push(arg)
            }
        }

        Arguments {
            flags,
            optional,
            main_command,
            positional,
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use crate::parser::{Arguments, CommandParser};

    #[test]
    fn test_parse_command() {
        let args = Vec::from(["a.exe", "put", "text=Hello World!", "-c", "-s", "mode=1", "-e", "environment=java", "--release", "box-1"]);

        let mut flags: Vec<String> = Vec::new();
        flags.push("c".into());
        flags.push("release".into());

        let mut optional: HashMap<String, String> = HashMap::new();
        optional.insert("s".into(), "mode=1".into());
        optional.insert("e".into(), "environment=java".into());
        optional.insert("text".into(), "Hello World!".into());

        let mut positional: Vec<String> = Vec::new();
        positional.push("box-1".into());

        let command = Arguments {
            flags,
            optional,
            main_command: Some("put".into()),
            positional,
        };
        let command_by_new = CommandParser::from_strings(args);

        assert_eq!(command, command_by_new);
    }
}
