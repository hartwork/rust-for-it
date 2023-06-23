// This file is part of the rust-for-it project.
//
// Copyright (c) 2023 Sebastian Pipping <sebastian@pipping.org>
// SPDX-License-Identifier: MIT

use clap::{command, Arg, ArgAction, Command};
use lazy_static::lazy_static;
use regex::Regex;

use super::network::TimeoutSeconds;

fn parse_service_syntax(text: &str) -> Result<String, String> {
    // Note: We are not using .to_socket_addrs() here because that
    //       would do DNS queries, already.
    static PATTERN: &str = r"^(\[[0-9a-fA-F.:]+\]|[^:]+):([1-9][0-9]{0,4})$";
    lazy_static! {
        static ref MATCHER: Regex = Regex::new(&PATTERN).unwrap();
    }
    match MATCHER.find(text) {
        Some(_) => Ok(text.to_string()),
        _ => Err(format!("does not match regular expression \"{PATTERN}\".")),
    }
}

#[cfg(test)]
#[test]
fn test_parse_service_syntax_for_valid() {
    assert_eq!(
        parse_service_syntax("[::1]:123"),
        Ok(String::from("[::1]:123"))
    );
    assert_eq!(
        parse_service_syntax("127.0.0.1:631"),
        Ok(String::from("127.0.0.1:631"))
    );
    assert_eq!(parse_service_syntax("h:1"), Ok(String::from("h:1")));
}

#[cfg(test)]
#[test]
fn test_parse_service_syntax_for_invalid() {
    let expected_error = Err(String::from(
        "does not match regular expression \"^(\\[[0-9a-fA-F.:]+\\]|[^:]+):([1-9][0-9]{0,4})$\".",
    ));
    assert_eq!(parse_service_syntax("h:123456"), expected_error);
    assert_eq!(parse_service_syntax("no colon"), expected_error);
    assert_eq!(parse_service_syntax(":123"), expected_error);
}

pub(crate) fn command() -> Command {
    command!()
        .arg(
            Arg::new("quiet")
                .action(ArgAction::SetTrue)
                .long("quiet")
                .short('q')
                .help("Do not output any status messages"),
        )
        .arg(
            Arg::new("strict")
                .action(ArgAction::SetTrue)
                .long("strict")
                .short('S')
                .help("Only execute <command> if all services are found available [default: always executes]"),
        )
        .arg(
            Arg::new("timeout_seconds")
                .long("timeout")
                .short('t')
                .value_name("seconds")
                .default_value("15")
                .help("Timeout in seconds, 0 for no timeout")
                .value_parser(clap::value_parser!(TimeoutSeconds)),
        )
        .arg(
            Arg::new("services")
                .action(ArgAction::Append)
                .long("service")
                .short('s')
                .value_name("host:port")
                .value_parser(parse_service_syntax)
                .num_args(0..)
                .help("Service to test via the TCP protocol; can be passed multiple times"),
        )
        .arg(
            Arg::new("command")
                .num_args(0..)
                .help("Command to run after waiting;\nincludes command arguments, resolved against ${PATH}"),
        )
}

#[cfg(test)]
#[test]
fn test_command_for_defaults() {
    let matches = command().get_matches_from(["rust-for-it"]);
    assert_eq!(*matches.get_one::<bool>("quiet").unwrap(), false);
    assert_eq!(*matches.get_one::<bool>("strict").unwrap(), false);
    assert_eq!(
        *matches
            .get_one::<TimeoutSeconds>("timeout_seconds")
            .unwrap(),
        15
    );
    assert!(matches
        .get_many::<String>("services")
        .unwrap_or_default()
        .next()
        .is_none());
    assert!(matches
        .get_many::<String>("command")
        .unwrap_or_default()
        .next()
        .is_none());
}

#[cfg(test)]
#[test]
fn test_command_for_non_defaults_long_or_positional() {
    let matches = command().get_matches_from([
        "rust-for-it",
        "--quiet",
        "--strict",
        "--timeout",
        "123",
        "--service",
        "one:1",
        "--service",
        "two:2",
        "--",
        "echo",
        "hello",
        "--",
        "world",
    ]);

    assert_eq!(*matches.get_one::<bool>("quiet").unwrap(), true);
    assert_eq!(*matches.get_one::<bool>("strict").unwrap(), true);
    assert_eq!(
        *matches
            .get_one::<TimeoutSeconds>("timeout_seconds")
            .unwrap(),
        123
    );

    let actual_services: Vec<_> = matches
        .get_many::<String>("services")
        .unwrap_or_default()
        .map(|e| e.as_str())
        .collect();
    assert_eq!(actual_services, ["one:1", "two:2"]);

    let actual_command: Vec<_> = matches
        .get_many::<String>("command")
        .unwrap_or_default()
        .map(|e| e.as_str())
        .collect();
    assert_eq!(actual_command, ["echo", "hello", "--", "world"]);
}

#[cfg(test)]
#[test]
fn test_command_for_non_defaults_short() {
    let matches = command().get_matches_from([
        "rust-for-it",
        "-q",
        "-t",
        "123",
        "-s",
        "one:1",
        "-s",
        "two:2",
    ]);

    assert_eq!(*matches.get_one::<bool>("quiet").unwrap(), true);
    assert_eq!(
        *matches
            .get_one::<TimeoutSeconds>("timeout_seconds")
            .unwrap(),
        123
    );

    let actual_services: Vec<_> = matches
        .get_many::<String>("services")
        .unwrap_or_default()
        .map(|e| e.as_str())
        .collect();
    assert_eq!(actual_services, ["one:1", "two:2"]);
}
