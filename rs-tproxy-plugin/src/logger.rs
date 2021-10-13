use std::convert::TryFrom;

use log::{Log, SetLoggerError};
use serde::{Deserialize, Serialize};

mod buildin {
    extern "C" {
        pub fn log_enabled(ptr: *const u8, len: u32) -> i32;
        pub fn log_log(ptr: *const u8, len: u32);
        pub fn log_flush();
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata<'a> {
    pub level: &'a str,
    pub target: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Record<'a> {
    pub level: &'a str,
    pub target: &'a str,
    pub content: String,
    pub mount_path: Option<&'a str>,
    pub file: Option<&'a str>,
    pub line: Option<u32>,
}

static LOGGER: &dyn Log = &Logger();
struct Logger();

pub fn setup_logger() -> Result<(), SetLoggerError> {
    log::set_logger(LOGGER)
}

impl Log for Logger {
    // log level cannot be set by plugin
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        let meta: Metadata = metadata.into();
        let data = serde_json::to_vec(&meta).unwrap();
        unsafe { buildin::log_enabled(data.as_ptr(), data.len() as u32) != 0 }
    }

    fn log(&self, record: &log::Record) {
        let re: Record = record.into();
        let data = serde_json::to_vec(&re).unwrap();
        unsafe { buildin::log_log(data.as_ptr(), data.len() as u32) }
    }

    /// Flushes any buffered records.
    fn flush(&self) {
        unsafe { buildin::log_flush() }
    }
}

impl<'a> Record<'a> {
    pub fn build(&self, args: std::fmt::Arguments<'a>) -> anyhow::Result<log::Record<'a>> {
        Ok(log::Record::builder()
            .level(
                self.level
                    .parse()
                    .map_err(|err| anyhow::anyhow!("fail to parse level: {}", err))?,
            )
            .target(self.target)
            .args(args)
            .module_path(self.mount_path)
            .file(self.file)
            .line(self.line)
            .build())
    }
}

impl<'a> From<&'a log::Metadata<'a>> for Metadata<'a> {
    fn from(meta: &'a log::Metadata) -> Self {
        Self {
            level: meta.level().as_str(),
            target: meta.target(),
        }
    }
}

impl<'a> From<&'a log::Record<'a>> for Record<'a> {
    fn from(record: &'a log::Record<'a>) -> Self {
        Self {
            level: record.level().as_str(),
            target: record.target(),
            content: record.args().to_string(),
            mount_path: record.module_path(),
            file: record.file(),
            line: record.line(),
        }
    }
}

impl<'a> TryFrom<Metadata<'a>> for log::Metadata<'a> {
    type Error = anyhow::Error;

    fn try_from(meta: Metadata<'a>) -> Result<Self, Self::Error> {
        Ok(Self::builder()
            .level(
                meta.level
                    .parse()
                    .map_err(|err| anyhow::anyhow!("fail to parse level: {}", err))?,
            )
            .target(meta.target)
            .build())
    }
}
