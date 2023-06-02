// This file is part of the rust-for-it project.
//
// Copyright (c) 2023 Sebastian Pipping <sebastian@pipping.org>
// SPDX-License-Identifier: MIT

use crate::exec::run_command;
use crate::network::wait_for_service;
use std::process::exit;

mod command_line_parser;
mod exec;
mod network;

fn main() {
    let matches = command_line_parser::command().get_matches();

    let timeout_seconds = *matches.get_one("timeout_seconds").unwrap();
    let strict = *matches.get_one::<bool>("strict").unwrap();
    let verbose = !*matches.get_one::<bool>("quiet").unwrap();
    let services = matches.get_many::<String>("services").unwrap_or_default();
    let mut command_argv = matches.get_many::<String>("command").unwrap_or_default();

    let mut success = true;

    for host_and_port in services {
        let wait_result = wait_for_service(host_and_port, timeout_seconds, verbose);
        success &= wait_result.is_ok();
    }

    let command_opt = command_argv.next();
    let command_should_be_run = (!strict || success) && command_opt.is_some();
    let mut exit_code: i32 = if success { 0 } else { 1 };

    if command_should_be_run {
        let command = command_opt.unwrap();
        let args = command_argv.map(|e| e.as_str()).collect();
        exit_code = run_command(command, args, verbose);
    }

    exit(exit_code);
}
