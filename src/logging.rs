// This file is part of the rust-for-it project.
//
// Copyright (c) 2023 Sebastian Pipping <sebastian@pipping.org>
// SPDX-License-Identifier: MIT

use log::{set_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record};

static CUSTOM_LOG: CustomLog = CustomLog {};

struct CustomLog {}

impl Log for CustomLog {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if record.level() == Level::Error {
            eprintln!("{}", record.args());
        } else {
            println!("{}", record.args());
        }
    }

    fn flush(&self) {}
}

pub(crate) fn activate(max_log_level: LevelFilter) {
    match set_logger(&CUSTOM_LOG) {
        Ok(_) => {
            set_max_level(max_log_level);
        }
        Err(error) => {
            eprintln!("Failed to initialize logging, error {:?}.", error);
        }
    }
}
