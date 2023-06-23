// This file is part of the rust-for-it project.
//
// Copyright (c) 2023 Sebastian Pipping <sebastian@pipping.org>
// SPDX-License-Identifier: MIT

use crate::exec::run_command;
use crate::network::{wait_for_service, TimeoutSeconds};
use anstream::RawStream;
use clap::{ArgMatches, ColorChoice};
use log::{set_max_level, LevelFilter};
use std::env;
use std::env::args_os;
use std::ffi::OsString;
use std::process::exit;
use std::thread::{spawn, JoinHandle};

mod command_line_parser;
mod exec;
mod logging;
mod network;

fn main() {
    logging::activate(LevelFilter::Info);

    let argv = args_os();
    let stdout: &mut dyn RawStream = &mut std::io::stdout();
    let stderr: &mut dyn RawStream = &mut std::io::stderr();
    let color_choice = if env::var("NO_COLOR").unwrap_or(String::new()).is_empty() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    let exit_code = middle_main(argv, stdout, stderr, color_choice);
    exit(exit_code);
}

fn middle_main<I, T>(
    argv: I,
    stdout: &mut dyn RawStream,
    stderr: &mut dyn RawStream,
    color_choice: ColorChoice,
) -> i32
where
    // to match clap::Command.get_matches_from
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let clap_result = command_line_parser::command()
        .color(color_choice)
        .try_get_matches_from(argv);
    match clap_result {
        Ok(matches) => innermost_main(matches),
        Err(e) => {
            let target: &mut dyn RawStream = if e.use_stderr() { stderr } else { stdout };
            let use_color: bool = match color_choice {
                ColorChoice::Always => true,
                ColorChoice::Never => false,
                ColorChoice::Auto => target.is_terminal(),
            };
            let rendered = e.render();
            if use_color {
                let _ = write!(target, "{}", rendered.ansi());
            } else {
                let _ = write!(target, "{}", rendered);
            }
            e.exit_code()
        }
    }
}

#[cfg(test)]
fn capture_main<I, T>(argv: I) -> (i32, String, String)
where
    // to match clap::Command.get_matches_from
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut stdout = anstream::Buffer::new();
    let mut stderr = anstream::Buffer::new();
    let color_choice = ColorChoice::Never;

    let exit_code = middle_main(argv, &mut stdout, &mut stderr, color_choice);

    let stdout = String::from_utf8(stdout.as_bytes().to_vec()).expect("UTF-8 decode error");
    let stderr = String::from_utf8(stderr.as_bytes().to_vec()).expect("UTF-8 decode error");

    (exit_code, stdout, stderr)
}

#[cfg(test)]
#[test]
fn test_middle_main() {
    use indoc::indoc;
    use std::net::TcpListener;

    assert_eq!(
        capture_main(["rust-for-it", "--help"]),
        (
            0,
            String::from(indoc! {"
                Wait for one or more services to be available before executing a command.

                Usage: rust-for-it [OPTIONS] [command]...

                Arguments:
                  [command]...  Command to run after waiting;
                                includes command arguments, resolved against ${PATH}

                Options:
                  -q, --quiet                     Do not output any status messages
                  -S, --strict                    Only execute <command> if all services are found available [default: always executes]
                  -t, --timeout <seconds>         Timeout in seconds, 0 for no timeout [default: 15]
                  -s, --service [<host:port>...]  Service to test via the TCP protocol; can be passed multiple times
                  -h, --help                      Print help
                  -V, --version                   Print version
                "
            }),
            String::new()
        )
    );
    assert_eq!(
        capture_main(["rust-for-it", "--version"]),
        (0, String::from("rust-for-it 2.0.0\n"), String::new())
    );

    // Does bad usage produce exit code 2?
    assert!(matches!(
        capture_main(["rust-for-it", "--no-such-argument"]),
        (2, _, _)
    ));

    let port;
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        port = listener.local_addr().unwrap().port();

        // Do available services produce exit code 0?
        assert!(matches!(
            capture_main(["rust-for-it", "-s", format!("127.0.0.1:{port}").as_str()]),
            (0, _, _)
        ));

        // Is the exit code forwarded properly?
        assert!(matches!(
            capture_main([
                "rust-for-it",
                "-s",
                format!("127.0.0.1:{port}").as_str(),
                "--",
                "sh",
                "-c",
                "exit 123"
            ]),
            (123, _, _)
        ));

        // Is the command executed and the exit code forwarded properly even with --strict?
        assert!(matches!(
            capture_main([
                "rust-for-it",
                "--strict",
                "-s",
                format!("127.0.0.1:{port}").as_str(),
                "--",
                "sh",
                "-c",
                "exit 123"
            ]),
            (123, _, _)
        ));

        // NOTE: The listener stops listening when going out of scope
    }

    // Do unavailable services produce exit code 1?
    assert!(matches!(
        capture_main([
            "rust-for-it",
            "-t1",
            "-s",
            format!("127.0.0.1:{port}").as_str()
        ]),
        (1, _, _)
    ));
    assert!(matches!(
        capture_main([
            "rust-for-it",
            "-t1",
            "-s",
            format!("127.0.0.1:{port}").as_str(),
            "--",
            "sh",
            "-c",
            "exit 123"
        ]),
        (123, _, _)
    ));

    // Does --strict prevent the execution of the command properly?
    assert!(matches!(
        capture_main([
            "rust-for-it",
            "--strict",
            "-t1",
            "-s",
            format!("127.0.0.1:{port}").as_str(),
            "--",
            "sh",
            "-c",
            "exit 123"
        ]),
        (1, _, _)
    ));
}

fn innermost_main(matches: ArgMatches) -> i32 {
    let timeout_seconds: TimeoutSeconds = *matches.get_one("timeout_seconds").unwrap();
    let strict = *matches.get_one::<bool>("strict").unwrap();
    let verbose = !*matches.get_one::<bool>("quiet").unwrap();
    let services = matches.get_many::<String>("services").unwrap_or_default();
    let mut command_argv = matches.get_many::<String>("command").unwrap_or_default();

    if !verbose {
        set_max_level(LevelFilter::Off);
    }

    let mut success = true;
    let mut threads: Vec<JoinHandle<bool>> = Vec::new();

    for host_and_port in services {
        let host_and_port = host_and_port.clone();
        let timeout_seconds = timeout_seconds.clone();

        let thread = spawn(move || wait_for_service(&host_and_port, timeout_seconds).is_ok());

        threads.push(thread);
    }

    for thread in threads {
        success &= thread.join().unwrap_or(false);
    }

    let command_opt = command_argv.next();
    let command_should_be_run = (!strict || success) && command_opt.is_some();
    let mut exit_code: i32 = if success { 0 } else { 1 };

    if command_should_be_run {
        let command = command_opt.unwrap();
        let args = command_argv.map(|e| e.as_str()).collect();
        exit_code = run_command(command, args);
    }

    exit_code
}
