// This file is part of the rust-for-it project.
//
// Copyright (c) 2023 Sebastian Pipping <sebastian@pipping.org>
// SPDX-License-Identifier: MIT

use log::{kv::ToValue, kv::Value, set_logger, set_max_level, LevelFilter, Log, Metadata, Record};

static CUSTOM_LOG: CustomLog = CustomLog {};

struct CustomLog {}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum SubLevel {
    Starting,
    Succeeded,
    Failed,
}

impl From<u64> for SubLevel {
    fn from(value: u64) -> Self {
        match value {
            0 => SubLevel::Starting,
            1 => SubLevel::Succeeded,
            _ => SubLevel::Failed,
        }
    }
}

impl ToValue for SubLevel {
    fn to_value(&self) -> Value {
        match self {
            SubLevel::Starting => 0u64.to_value(),
            SubLevel::Succeeded => 1u64.to_value(),
            SubLevel::Failed => 2u64.to_value(),
        }
    }
}

#[cfg(test)]
#[test]
fn test_sublevel_casting() {
    assert_eq!(SubLevel::Starting.to_value().to_u64().unwrap(), 0);
    assert_eq!(SubLevel::Succeeded.to_value().to_u64().unwrap(), 1);
    assert_eq!(SubLevel::Failed.to_value().to_u64().unwrap(), 2);

    assert_eq!(SubLevel::from(0), SubLevel::Starting);
    assert_eq!(SubLevel::from(1), SubLevel::Succeeded);
    assert_eq!(SubLevel::from(2), SubLevel::Failed);
    assert_eq!(SubLevel::from(3), SubLevel::Failed);
}

impl Log for CustomLog {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let value: Value = record
            .key_values()
            .get("sublevel".into())
            .unwrap_or(Value::from(SubLevel::Failed as u64));
        let sublevel = SubLevel::from(value.to_u64().expect("malformed sublevel"));

        match sublevel {
            SubLevel::Starting => {
                println!("[*] {}", record.args());
            }
            SubLevel::Succeeded => {
                println!("[+] {}", record.args());
            }
            SubLevel::Failed => {
                eprintln!("[-] {}", record.args());
            }
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
