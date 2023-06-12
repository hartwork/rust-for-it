// This file is part of the rust-for-it project.
//
// Copyright (c) 2023 Sebastian Pipping <sebastian@pipping.org>
// SPDX-License-Identifier: MIT

use crate::exec::run_command;
use crate::network::{wait_for_service, TimeoutSeconds};
use clap::ArgMatches;
use std::env::args_os;
use std::ffi::OsString;
use std::process::exit;
use std::thread::{spawn, JoinHandle};

mod command_line_parser;
mod exec;
mod network;

// Matches the two internal constants from [clap_builder-4.3.1]/src/util/mod.rs
const SUCCESS_CODE: i32 = 0;
const USAGE_CODE: i32 = 2;

fn main() {
    exit(middle_main(args_os()));
}

fn middle_main<I, T>(argv: I) -> i32
where
    // to match clap::Command.get_matches_from
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let clap_result = command_line_parser::command().try_get_matches_from(argv);
    match clap_result {
        Ok(matches) => innermost_main(matches),
        Err(e) => {
            // This mimics clap::Error.exit minus the call to safe_exit
            let _ = e.print();
            if e.use_stderr() {
                USAGE_CODE
            } else {
                SUCCESS_CODE
            }
        }
    }
}

#[cfg(test)]
#[test]
fn test_middle_main() {
    use std::net::TcpListener;

    assert_eq!(middle_main(["rust-for-it", "--help"]), SUCCESS_CODE);
    assert_eq!(middle_main(["rust-for-it", "--version"]), SUCCESS_CODE);

    // Does bad usage produce exit code 2?
    assert_eq!(
        middle_main(["rust-for-it", "--no-such-argument"]),
        USAGE_CODE
    );

    let port;
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        port = listener.local_addr().unwrap().port();

        // Do available services produce exit code 0?
        assert_eq!(
            middle_main(["rust-for-it", "-s", format!("127.0.0.1:{port}").as_str()]),
            SUCCESS_CODE,
        );

        // Is the exit code forwarded properly?
        assert_eq!(
            middle_main([
                "rust-for-it",
                "-s",
                format!("127.0.0.1:{port}").as_str(),
                "--",
                "sh",
                "-c",
                "exit 123"
            ]),
            123,
        );

        // Is the command executed and the exit code forwarded properly even with --strict?
        assert_eq!(
            middle_main([
                "rust-for-it",
                "--strict",
                "-s",
                format!("127.0.0.1:{port}").as_str(),
                "--",
                "sh",
                "-c",
                "exit 123"
            ]),
            123,
        );

        // NOTE: The listener stops listening when going out of scope
    }

    // Do unavailable services produce exit code 1?
    assert_eq!(
        middle_main([
            "rust-for-it",
            "-t1",
            "-s",
            format!("127.0.0.1:{port}").as_str()
        ]),
        1,
    );
    assert_eq!(
        middle_main([
            "rust-for-it",
            "-t1",
            "-s",
            format!("127.0.0.1:{port}").as_str(),
            "--",
            "sh",
            "-c",
            "exit 123"
        ]),
        123,
    );

    // Does --strict prevent the execution of the command properly?
    assert_eq!(
        middle_main([
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
        1,
    );
}

fn innermost_main(matches: ArgMatches) -> i32 {
    let timeout_seconds: TimeoutSeconds = *matches.get_one("timeout_seconds").unwrap();
    let strict = *matches.get_one::<bool>("strict").unwrap();
    let verbose = !*matches.get_one::<bool>("quiet").unwrap();
    let services = matches.get_many::<String>("services").unwrap_or_default();
    let mut command_argv = matches.get_many::<String>("command").unwrap_or_default();

    let mut success = true;
    let mut threads: Vec<JoinHandle<bool>> = Vec::new();

    for host_and_port in services {
        let host_and_port = host_and_port.clone();
        let timeout_seconds = timeout_seconds.clone();
        let verbose = verbose.clone();

        let thread =
            spawn(move || wait_for_service(&host_and_port, timeout_seconds, verbose).is_ok());

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
        exit_code = run_command(command, args, verbose);
    }

    exit_code
}
