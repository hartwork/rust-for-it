// This file is part of the rust-for-it project.
//
// Copyright (c) 2023 Sebastian Pipping <sebastian@pipping.org>
// SPDX-License-Identifier: MIT

use std::io::ErrorKind;
use subprocess::Exec;
use subprocess::ExitStatus;
use subprocess::PopenError;
use subprocess::Result as PopenResult;

fn exit_code_from(exit_status: ExitStatus) -> i32 {
    match exit_status {
        ExitStatus::Exited(exit_code) => exit_code as i32,
        ExitStatus::Signaled(signal) => (128 + signal) as i32,
        _ => 255,
    }
}

#[cfg(test)]
#[test]
fn test_exit_code_from() {
    assert_eq!(exit_code_from(ExitStatus::Exited(123)), 123);
    assert_eq!(exit_code_from(ExitStatus::Signaled(2)), 130);
    assert_eq!(exit_code_from(ExitStatus::Other(123)), 255);
    assert_eq!(exit_code_from(ExitStatus::Undetermined), 255);
}

fn process_popen_result(
    popen_result: PopenResult<ExitStatus>,
    command: &str,
    verbose: bool,
) -> i32 {
    match popen_result {
        Ok(exit_status) => exit_code_from(exit_status),
        Err(PopenError::IoError(error)) if error.kind() == ErrorKind::PermissionDenied => {
            if verbose {
                println!("[-] Command '{command}' could not be run: permission denied.");
            }
            126
        }
        Err(PopenError::IoError(error)) if error.kind() == ErrorKind::NotFound => {
            if verbose {
                println!("[-] Command '{command}' not found.");
            }
            127
        }
        Err(error) => {
            if verbose {
                println!("[-] Command '{command}' failed with unexpected error: {error}.");
            }
            255
        }
    }
}

#[cfg(test)]
#[test]
fn test_process_popen_result() {
    let command = "command1";
    for verbose in [true, false] {
        assert_eq!(
            process_popen_result(Ok(ExitStatus::Exited(123)), command, verbose.clone()),
            123
        );

        let permission_denied = std::io::Error::new(ErrorKind::PermissionDenied, "error1");
        assert_eq!(
            process_popen_result(
                Err(PopenError::IoError(permission_denied)),
                command,
                verbose.clone()
            ),
            126
        );

        let not_found = std::io::Error::new(ErrorKind::NotFound, "error2");
        assert_eq!(
            process_popen_result(
                Err(PopenError::IoError(not_found)),
                command,
                verbose.clone()
            ),
            127
        );

        let other_error = std::io::Error::new(ErrorKind::BrokenPipe, "error3");
        assert_eq!(
            process_popen_result(Err(PopenError::IoError(other_error)), command, verbose),
            255
        );
    }
}

pub fn run_command(command: &str, args: Vec<&str>, verbose: bool) -> i32 {
    let popen_result = Exec::cmd(command).args(args.as_slice()).join();
    process_popen_result(popen_result, command, verbose)
}

#[cfg(test)]
#[test]
fn test_run_command_for_good() {
    assert_eq!(run_command("sh", vec!["-c", "exit 0"], false), 0);
}

#[cfg(test)]
#[test]
fn test_run_command_for_bad() {
    assert_eq!(run_command("sh", vec!["-c", "exit 123"], false), 123);
}
