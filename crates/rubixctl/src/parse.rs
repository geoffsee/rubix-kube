use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckOptions {
    pub install_prerequisites: bool,
    pub pprof: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpTopic {
    Root,
    Check,
    Version,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Help(HelpTopic),
    Version,
    UnknownHelp,
    Check(CheckOptions),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    UnknownCommand,
    UnknownFlag,
    InvalidBoolean,
    TooManyArguments,
}
impl ParseError {
    pub fn message(self) -> &'static str {
        match self {
            Self::UnknownCommand => "unknown command; use rubixctl --help",
            Self::UnknownFlag => "unknown flag for this command; use --help",
            Self::InvalidBoolean => "invalid boolean flag value; use true or false",
            Self::TooManyArguments => "command input exceeds the supported byte or argument limit",
        }
    }
}
fn environment_bool(environment: &BTreeMap<String, String>, key: &str) -> bool {
    matches!(
        environment.get(key).map(String::as_str),
        Some("true" | "1" | "yes")
    )
}
fn boolean(value: &str) -> Result<bool, ParseError> {
    match value {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Ok(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Ok(false),
        _ => Err(ParseError::InvalidBoolean),
    }
}
/// Parses management arguments only. No filesystem, environment or host probes.
/// Errors never reproduce attacker-controlled argument values.
pub fn parse_command(
    args: &[String],
    environment: &BTreeMap<String, String>,
) -> Result<Command, ParseError> {
    if args.len() > 256
        || args
            .iter()
            .map(String::len)
            .try_fold(0usize, usize::checked_add)
            .is_none_or(|n| n > 65536)
    {
        return Err(ParseError::TooManyArguments);
    }
    let mut topic = HelpTopic::Root;
    let mut command_index = None;
    let mut help_command = false;
    let mut cursor = 0;
    // Cobra finds a command before registering its help flag. Root flags without
    // an attached value consume the next token while locating the command.
    while let Some(arg) = args.get(cursor) {
        if arg == "--" {
            break;
        }
        if arg.starts_with('-') && arg != "-" {
            cursor += if arg.contains('=') { 1 } else { 2 };
            continue;
        }
        topic = match arg.as_str() {
            "check" => HelpTopic::Check,
            "version" => HelpTopic::Version,
            "help" => {
                help_command = true;
                HelpTopic::Root
            },
            _ => return Err(ParseError::UnknownCommand),
        };
        command_index = Some(cursor);
        break;
    }
    let mut options = CheckOptions {
        install_prerequisites: environment_bool(environment, "KUBESOLO_INSTALL_PREREQS"),
        pprof: environment_bool(environment, "KUBESOLO_PPROF_SERVER"),
    };
    let mut help = false;
    let mut positional = Vec::new();
    let mut literal = false;
    for (i, arg) in args.iter().enumerate() {
        if Some(i) == command_index {
            continue;
        }
        if literal {
            positional.push(arg.as_str());
            continue;
        }
        if arg == "--" {
            literal = true;
            continue;
        }
        if !arg.starts_with('-') || arg == "-" {
            positional.push(arg.as_str());
            continue;
        }
        if let Some(flag) = arg.strip_prefix("--") {
            let (name, value) = flag
                .split_once('=')
                .map_or((flag, None), |(n, v)| (n, Some(v)));
            let slot = match name {
                "help" => &mut help,
                "install-prereqs" if topic == HelpTopic::Check => {
                    &mut options.install_prerequisites
                },
                "pprof-server" if topic == HelpTopic::Check => &mut options.pprof,
                _ => return Err(ParseError::UnknownFlag),
            };
            *slot = value.map_or(Ok(true), boolean)?;
        } else {
            let (letters, value) = arg[1..]
                .split_once('=')
                .map_or((&arg[1..], None), |(n, v)| (n, Some(v)));
            if letters.is_empty() || !letters.bytes().all(|b| b == b'h') {
                return Err(ParseError::UnknownFlag);
            }
            help = value.map_or(Ok(true), boolean)?;
        }
    }
    if help {
        return Ok(Command::Help(topic));
    }
    if help_command {
        return Ok(match positional.first().copied() {
            None => Command::Help(HelpTopic::Root),
            Some("check") => Command::Help(HelpTopic::Check),
            Some("version") => Command::Help(HelpTopic::Version),
            _ => Command::UnknownHelp,
        });
    }
    Ok(match topic {
        HelpTopic::Check => Command::Check(options),
        HelpTopic::Version => Command::Version,
        HelpTopic::Root => Command::Help(HelpTopic::Root),
    })
}
