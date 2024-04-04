use std::collections::HashMap;

#[derive(Debug, Eq)]
pub struct Arguments {
    pub flags: Vec<String>,
    pub positional: Vec<String>,
    pub main_command: Option<String>,
    pub optional: HashMap<String, String>,
}

impl PartialEq for Arguments {
    fn eq(&self, other: &Self) -> bool {

        let sorted_flags_self = {
            let mut tmp = self.flags.clone();
            tmp.sort();
            tmp
        };

        let sorted_flags_other = {
            let mut tmp = other.flags.clone();
            tmp.sort();
            tmp
        };

        sorted_flags_self == sorted_flags_other
        && self.positional == other.positional
        && self.main_command == other.main_command
        && self.optional == other.optional
    }
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
            if arg.starts_with('-') {
                let skip_chr = arg.get_skip_chr();
                if skip_chr == -1 {
                    continue;
                }

                if let Some(next_arg) = buffer.take().or_else(|| iter.next().map(|arg| arg.into())) {
                    let next_skip_chr = next_arg.get_skip_chr();

                    if next_skip_chr > 0 {
                        buffer = Some(next_arg);
                        flags.push(arg[skip_chr as usize..].into());
                    } else {
                        optional.insert(arg[skip_chr as usize..].into(), next_arg);
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

pub trait SkipChr {
    fn get_skip_chr(&self) -> i8;
}

impl SkipChr for str {
    fn get_skip_chr(&self) -> i8 {
        let text_str: String = self.into();
        if !text_str.starts_with('-') {
            return -1;
        }

        if text_str.len() < 2 {
            return -1;
        }

        let is_start_with_dash_dash = text_str.starts_with("--");

        if is_start_with_dash_dash {
            if text_str.len() < 3 {
                return -1;
            }
            return 2;
        }
        1
    }
}




#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use crate::parser::{Arguments, CommandParser, SkipChr};

    #[test]
    fn test_skip_chr() {
        let command_1 = "--a";
        let command_2 = "--";
        let command_3 = "-a";
        let command_4 = "-";

        assert_eq!(command_1.get_skip_chr(), 2);
        assert_eq!(command_2.get_skip_chr(), -1);
        assert_eq!(command_3.get_skip_chr(), 1);
        assert_eq!(command_4.get_skip_chr(), -1);
    }

    #[test]
    fn test_parse_command() {
        let args = Vec::from(["a.exe", "put", "text=Hello World!", "--release", "-c", "-s", "mode=1", "-e", "environment=java", "box-1", "box-2"]);

        let mut flags: Vec<String> = Vec::new();
        flags.push("c".into());
        flags.push("release".into());

        let mut optional: HashMap<String, String> = HashMap::new();
        optional.insert("s".into(), "mode=1".into());
        optional.insert("e".into(), "environment=java".into());
        optional.insert("text".into(), "Hello World!".into());

        let mut positional: Vec<String> = Vec::new();
        positional.push("box-1".into());
        positional.push("box-2".into());

        let command = Arguments {
            flags,
            optional,
            main_command: Some("put".into()),
            positional,
        };
        let command_by_from = CommandParser::from_strings(args);

        assert_eq!(command, command_by_from);
    }
}
