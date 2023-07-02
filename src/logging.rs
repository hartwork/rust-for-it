// This file is part of the rust-for-it project.
//
// Copyright (c) 2023 Sebastian Pipping <sebastian@pipping.org>
// SPDX-License-Identifier: MIT

use anstream::RawStream;
use log::{kv::ToValue, kv::Value, set_logger, set_max_level, LevelFilter, Log, Metadata, Record};
use once_cell::sync::Lazy;

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::thread::ThreadId;

static CUSTOM_LOG: CustomLog = CustomLog {};

struct CustomLog {}

struct LogConfig {
    stdout: Option<Arc<Mutex<&'static mut dyn RawStream>>>,
    stderr: Option<Arc<Mutex<&'static mut dyn RawStream>>>,
}

static mut LOG_CONFIG: Mutex<LogConfig> = Mutex::new(LogConfig {
    stdout: None,
    stderr: None,
});

static mut LOG_ACTIVE: Mutex<()> = Mutex::new(());

static mut INCLUDED_THREADS: Lazy<Mutex<HashSet<ThreadId>>> =
    Lazy::new(|| Mutex::new(HashSet::<ThreadId>::new()));

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

impl Log for CustomLog {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let thread_id = thread::current().id();
        {
            let included_threads = unsafe { INCLUDED_THREADS.lock() }.expect("poisoned lock");
            if !included_threads.contains(&thread_id) {
                return;
            }
        }

        let value: Value = record
            .key_values()
            .get("sublevel".into())
            .unwrap_or(Value::from(SubLevel::Failed as u64));
        let sublevel = SubLevel::from(value.to_u64().expect("malformed sublevel"));

        let icon = match sublevel {
            SubLevel::Starting => '*',
            SubLevel::Succeeded => '+',
            SubLevel::Failed => '-',
        };

        let mut log_config = unsafe { LOG_CONFIG.lock() }.expect("poisoned lock");

        let target: &mut Option<Arc<Mutex<&'static mut dyn RawStream>>> = match sublevel {
            SubLevel::Starting | SubLevel::Succeeded => &mut log_config.stdout,
            SubLevel::Failed => &mut log_config.stderr,
        };

        if let Some(target) = target.as_mut() {
            let mut target = target.lock().expect("poisoned lock");
            let target: &mut dyn RawStream = *target;

            let _ = writeln!(target, "[{}] {}", icon, record.args());
        }
    }

    fn flush(&self) {}
}

pub(crate) fn with_exclusive_logging<F, R>(
    max_log_level: LevelFilter,
    stdout: Arc<Mutex<&'static mut dyn RawStream>>,
    stderr: Arc<Mutex<&'static mut dyn RawStream>>,
    inner_function: F,
) -> R
where
    F: FnOnce() -> R,
{
    let _locked = unsafe { LOG_ACTIVE.lock() }.expect("poisoned lock");

    // NOTE: set_logger only ever succeeds *once* per process lifetime
    if let Err(error) = set_logger(&CUSTOM_LOG) {
        #[cfg(test)]
        let _ = error;

        #[cfg(not(test))]
        panic!("Failed to initialize logging, error {:?}.", error);
    }

    set_max_level(max_log_level);
    {
        let mut log_config = unsafe { LOG_CONFIG.lock() }.expect("poisoned lock");
        log_config.stdout = Some(stdout);
        log_config.stderr = Some(stderr);
    }

    let res = with_logging_for_current_thread(inner_function);

    set_max_level(LevelFilter::Off);
    {
        let mut log_config = unsafe { LOG_CONFIG.lock() }.expect("poisoned lock");
        log_config.stdout = None;
        log_config.stderr = None;
    }

    res
}

pub(crate) fn with_logging_for_current_thread<F, R>(inner_function: F) -> R
where
    F: FnOnce() -> R,
{
    let thread_id = thread::current().id();
    {
        let mut included_threads = unsafe { INCLUDED_THREADS.lock() }.expect("poisoned lock");
        included_threads.insert(thread_id);
    }

    let ret = inner_function();

    {
        let mut included_threads = unsafe { INCLUDED_THREADS.lock() }.expect("poisoned lock");
        included_threads.remove(&thread_id);
    }

    ret
}

#[cfg(test)]
mod tests {
    use anstream::RawStream;
    use extend_lifetime::extend_lifetime;
    use indoc::indoc;
    use log::kv::ToValue;
    use log::LevelFilter;
    use log::{error, info};

    use std::sync::Arc;
    use std::sync::Mutex;

    use super::with_exclusive_logging;
    use super::SubLevel;

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

    #[test]
    fn test_with_exclusive_logging() {
        let mut stdout_buffer = anstream::Buffer::new();
        let mut stderr_buffer = anstream::Buffer::new();
        let stdout: Arc<Mutex<&mut dyn RawStream>> =
            Arc::new(Mutex::new(unsafe { extend_lifetime(&mut stdout_buffer) }));
        let stderr: Arc<Mutex<&mut dyn RawStream>> =
            Arc::new(Mutex::new(unsafe { extend_lifetime(&mut stderr_buffer) }));

        let expected_result = 123;
        let actual_result = with_exclusive_logging(LevelFilter::Info, stdout, stderr, || -> i32 {
            info!(target: module_path!(), sublevel = SubLevel::Starting; "11111 11111");
            info!(target: module_path!(), sublevel = SubLevel::Succeeded; "22222 22222");
            error!("33333 33333");
            error!("44444 44444");
            expected_result
        });

        let stdout =
            String::from_utf8(stdout_buffer.as_bytes().to_vec()).expect("UTF-8 decode error");
        let stderr =
            String::from_utf8(stderr_buffer.as_bytes().to_vec()).expect("UTF-8 decode error");

        assert_eq!(
            stdout,
            indoc! {"
                [*] 11111 11111
                [+] 22222 22222
            "}
        );
        assert_eq!(
            stderr,
            indoc! {"
                [-] 33333 33333
                [-] 44444 44444
            "}
        );

        assert_eq!(actual_result, expected_result);
    }
}
