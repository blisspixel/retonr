//! Parse failures that name the bad argument, a usage line, and help.

use std::ffi::OsString;

use clap::error::{ContextKind, ErrorKind};

use crate::{contract::CommandName, failure::RunFailure};

/// One clap rejection, ready for text or the JSON usage envelope.
pub(crate) struct ParseFailure {
    pub(crate) failure: RunFailure,
    pub(crate) message: String,
    pub(crate) usage: String,
    pub(crate) more: String,
}

pub(crate) fn from_clap(arguments: &[OsString], error: &clap::Error) -> ParseFailure {
    let invalid = context(error, ContextKind::InvalidArg);
    let subcommand = context(error, ContextKind::InvalidSubcommand);
    let message = match error.kind() {
        ErrorKind::UnknownArgument => format!(
            "unexpected argument '{}'",
            invalid.as_deref().unwrap_or("argument")
        ),
        ErrorKind::InvalidSubcommand => format!(
            "unrecognized command '{}'",
            subcommand.as_deref().unwrap_or("command")
        ),
        ErrorKind::MissingRequiredArgument => invalid.as_ref().map_or_else(
            || "a command is required".to_owned(),
            |name| format!("missing required argument {name}"),
        ),
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => "a command is required".to_owned(),
        ErrorKind::InvalidValue => invalid.as_ref().map_or_else(
            || "a value is invalid".to_owned(),
            |name| format!("invalid value for {name}"),
        ),
        _ => "command input is invalid".to_owned(),
    };
    let usage = context(error, ContextKind::Usage)
        .map(|value| strip_usage_prefix(&plain(&value)))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "retonr <COMMAND>".to_owned());
    let command = command_token(arguments);
    let mut failure = RunFailure::usage_for(command_name(command.as_deref()));
    if let Some(diagnostic) = invalid.or(subcommand) {
        let diagnostic = plain(&diagnostic);
        if !diagnostic.is_empty() {
            failure.body = failure.body.with_diagnostic(&diagnostic);
        }
    }
    ParseFailure {
        failure,
        message: plain(&message),
        usage,
        more: help_pointer(command.as_deref()),
    }
}

fn context(error: &clap::Error, kind: ContextKind) -> Option<String> {
    error.get(kind).map(std::string::ToString::to_string)
}

fn strip_usage_prefix(usage: &str) -> String {
    usage
        .strip_prefix("Usage:")
        .or_else(|| usage.strip_prefix("usage:"))
        .map_or(usage, str::trim)
        .to_owned()
}

fn plain(text: &str) -> String {
    let mut clean = String::new();
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        if !character.is_control() || character == '\n' {
            clean.push(character);
        }
    }
    clean
}

fn command_token(arguments: &[OsString]) -> Option<String> {
    let mut saw_program = false;
    let mut skip_value = false;
    for argument in arguments {
        let Some(text) = argument.to_str() else {
            continue;
        };
        if !saw_program {
            saw_program = true;
            continue;
        }
        if skip_value {
            skip_value = false;
            continue;
        }
        if text == "--" {
            break;
        }
        if text == "--verbose" {
            continue;
        }
        if matches!(text, "--format" | "-f" | "--color" | "-D" | "--data-dir") {
            skip_value = true;
            continue;
        }
        if text.starts_with('-') {
            continue;
        }
        return Some(text.to_owned());
    }
    None
}

fn command_name(command: Option<&str>) -> CommandName {
    match command {
        Some("check") => CommandName::Check,
        Some("rewrite") => CommandName::Rewrite,
        Some("inspect") => CommandName::Inspect,
        Some("lint") => CommandName::Lint,
        Some("tui") => CommandName::Tui,
        Some("doctor") => CommandName::Doctor,
        Some("version") => CommandName::Version,
        Some("completions") => CommandName::Completions,
        Some("man") => CommandName::Man,
        Some("model") => CommandName::ModelList,
        _ => CommandName::Cli,
    }
}

fn help_pointer(command: Option<&str>) -> String {
    match command {
        Some("model") => "retonr model --help".to_owned(),
        Some(name)
            if matches!(
                name,
                "check"
                    | "rewrite"
                    | "inspect"
                    | "lint"
                    | "tui"
                    | "doctor"
                    | "version"
                    | "completions"
                    | "man"
            ) =>
        {
            format!("retonr {name} --help")
        }
        _ => "retonr --help".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, process::ExitCode};

    use clap::{Parser, error::ErrorKind};

    use crate::{args::Cli, contract::EXIT_USAGE};

    use super::from_clap;

    fn parse(arguments: &[&str]) -> super::ParseFailure {
        let owned: Vec<OsString> = arguments.iter().copied().map(OsString::from).collect();
        let error = Cli::try_parse_from(&owned).expect_err("usage error");
        assert_ne!(error.kind(), ErrorKind::DisplayHelp);
        assert_ne!(error.kind(), ErrorKind::DisplayVersion);
        from_clap(&owned, &error)
    }

    #[test]
    fn names_the_bad_argument_a_missing_command_and_a_missing_candidate() {
        let bogus = parse(&["retonr", "--bogus"]);
        assert!(
            bogus.message.contains("unexpected argument '--bogus'"),
            "{}",
            bogus.message
        );
        assert!(bogus.more.contains("retonr --help"), "{}", bogus.more);
        assert_eq!(bogus.failure.exit_code, ExitCode::from(EXIT_USAGE));

        let bare = parse(&["retonr"]);
        assert!(
            bare.message.contains("a command is required"),
            "{}",
            bare.message
        );
        assert_eq!(bare.failure.exit_code, ExitCode::from(EXIT_USAGE));

        let missing = parse(&["retonr", "check", "only-one"]);
        assert!(
            missing.message.contains("missing required argument"),
            "{}",
            missing.message
        );
        assert!(
            missing.more.contains("retonr check --help"),
            "{}",
            missing.more
        );

        let verbose = parse(&["retonr", "--verbose", "check", "only-one"]);
        assert!(
            verbose.message.contains("missing required argument"),
            "{}",
            verbose.message
        );
        assert!(
            verbose.more.contains("retonr check --help"),
            "{}",
            verbose.more
        );
        assert_eq!(verbose.failure.command, crate::contract::CommandName::Check);
    }
}
