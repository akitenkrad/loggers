//! # Loggers
//!
//! ## Installation
//! ```bash
//! cargo add loggers
//! ```
//!
//! # Examples
//! ```rust
//! use loggers::*;
//! let mut logger = Logger::new();
//! logger.add_logger(Box::new(CustomLogger::new(
//!     "test",
//!     "tests/output/system.log",
//! )));
//! logger.set_fallback(Box::new(CustomLogger::new(
//!     "default",
//!     "tests/output/system.log",
//! )));
//! log::set_boxed_logger(Box::new(logger)).expect("Failed to set logger");
//! log::set_max_level(log::LevelFilter::Trace);
//!
//! info!(target:"test", "Hello, world!");
//! debug!("Default");
//! ```

use chrono::{Local, SecondsFormat};
use std::{fs::File, io::prelude::*, path::Path};
pub struct Logger {
    loggers: Vec<Box<dyn log::Log>>,
    fallback: Option<Box<dyn log::Log>>,
}

impl Logger {
    pub fn new() -> Logger {
        return Logger {
            loggers: Vec::new(),
            fallback: None,
        };
    }

    /// add a CustomLogger to the logger
    /// # Arguments
    /// * `logger::CustomLogger` - The logger to add
    /// # Example
    /// ```
    /// # use crate::loggers::*;
    /// # use std::io::Write;
    /// let mut logger = Logger::new();
    /// logger.add_logger(Box::new(CustomLogger::new("test", "system.log")));
    /// ```
    pub fn add_logger(&mut self, logger: Box<dyn log::Log>) {
        self.loggers.push(logger);
    }

    /// set a fallback logger::CustomLogger
    /// # Arguments
    /// * `fallback::CustomLogger` - The fallback logger
    /// # Example
    /// ```
    /// # use crate::loggers::*;
    /// # use std::io::Write;
    /// let mut logger = Logger::new();
    /// logger.set_fallback(Box::new(CustomLogger::new("test", "system.log")));
    /// ```
    pub fn set_fallback(&mut self, fallback: Box<dyn log::Log>) {
        self.fallback = Some(fallback);
    }
}

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        return true;
    }

    fn log(&self, record: &log::Record) {
        let mut logged = false;

        for logger in &self.loggers {
            if logger.enabled(record.metadata()) {
                logger.log(record);
                logged = true;
            }
        }

        if !logged {
            if let Some(fallback) = &self.fallback {
                fallback.log(record);
            }
        }
    }

    fn flush(&self) {}
}

pub struct CustomLogger {
    target: String,
    filepath: Option<String>,
}

impl CustomLogger {
    pub fn new(target: &str, filepath: &str) -> CustomLogger {
        let path = Path::new(filepath);
        path.parent().map(|p| std::fs::create_dir_all(p).unwrap());
        File::create(filepath).unwrap();
        CustomLogger {
            target: target.to_string(),
            filepath: Some(filepath.to_string()),
        }
    }
}

impl log::Log for CustomLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if metadata.target() == self.target {
            return true;
        }
        return false;
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let log_json_text = format!(
            r#"{{"severity":"{}","timestamp":"{}","target":"{}","message":"{}"}}"#,
            record.level(),
            Local::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            self.target,
            record.args(),
        );
        let log_print_text = format!(
            "[{}] {} {} - {}",
            record.level().to_string().to_uppercase(),
            self.target,
            Local::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            record.args(),
        );

        match self.filepath {
            Some(ref filepath) => {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(filepath)
                    .unwrap();
                let mut bf = std::io::BufWriter::new(file);

                bf.write(log_json_text.as_bytes()).unwrap();
                bf.write(b"\n").unwrap();
            }
            None => {
                println!("Cannot open file {:?}", self.filepath);
            }
        }
        // if let Some(filepath) = &self.filepath {
        //     let file = std::fs::OpenOptions::new()
        //         .create(true)
        //         .append(true)
        //         .open(filepath)
        //         .unwrap();
        //     let mut bf = std::io::BufWriter::new(file);

        //     bf.write(log_json_text.as_bytes()).unwrap();
        //     bf.write(b"\n").unwrap();
        // }

        println!("{}", log_print_text);
    }

    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::{debug, info};
    use serde_json::Value;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    #[test]
    fn test_log() {
        let log_file = Path::new("tests/output/test.log");
        if log_file.exists() {
            std::fs::remove_file(log_file).unwrap();
        }

        let mut logger = Logger::new();
        logger.add_logger(Box::new(CustomLogger::new(
            "test",
            "tests/output/system.log",
        )));
        logger.set_fallback(Box::new(CustomLogger::new(
            "default",
            "tests/output/system.log",
        )));
        log::set_boxed_logger(Box::new(logger)).expect("Failed to set logger");
        log::set_max_level(log::LevelFilter::Trace);

        info!(target:"test", "Hello, world!");
        debug!("Default");

        let mut file = File::open("tests/output/system.log").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        let v: Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(v["severity"], "INFO");
        assert_eq!(v["target"], "test");
        assert_eq!(v["message"], "Hello, world!");
    }
}
