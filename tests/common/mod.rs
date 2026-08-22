//! Shared test helpers. Compiled into every test binary, so not every
//! helper is used by all of them.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// A fresh path under `tests/output/`, with any leftover from a previous run removed.
pub fn out(name: &str) -> PathBuf {
    let path = Path::new("tests/output").join(name);
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    path
}

/// Feed one record straight into a logger, bypassing the global `log` machinery
/// so that several tests can share a process.
pub fn emit(logger: &dyn log::Log, target: &str, level: log::Level, message: &str) {
    logger.log(
        &log::Record::builder()
            .args(format_args!("{message}"))
            .level(level)
            .target(target)
            .build(),
    );
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn lines(path: &Path) -> Vec<serde_json::Value> {
    read(path)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("invalid JSON line {l:?}: {e}")))
        .collect()
}
