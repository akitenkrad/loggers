//! # Loggers
//!
//! A small [`log`] backend that routes records to per-target log files, with an
//! optional fallback logger that catches everything else.
//!
//! Each record is appended to its file as a single line of JSON (JSON Lines) and
//! echoed to stdout in a human-readable form. Every logger that accepts a record
//! echoes it, so a record claimed by two loggers is printed twice; turn the echo
//! off with [`CustomLogger::with_stdout`].
//!
//! ## Installation
//! ```bash
//! cargo add loggers
//! ```
//!
//! # Examples
//! ```rust
//! use log::{debug, info};
//! use loggers::*;
//!
//! let mut logger = Logger::new();
//! logger.add_logger(Box::new(CustomLogger::new(
//!     "test",
//!     "tests/output/system.log",
//! )));
//! logger.set_fallback(Box::new(CustomLogger::catch_all(
//!     "tests/output/fallback.log",
//! )));
//! log::set_boxed_logger(Box::new(logger)).expect("Failed to set logger");
//! log::set_max_level(log::LevelFilter::Trace);
//!
//! // Routed to tests/output/system.log by its target.
//! info!(target: "test", "Hello, world!");
//! // No logger claims this target, so it goes to the fallback.
//! debug!("Default");
//! ```

use chrono::{Local, SecondsFormat};
use std::collections::HashMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};

/// A log file plus the lock that serialises writes to it.
type SharedFile = Arc<Mutex<File>>;

/// Every [`CustomLogger`] pointed at the same path shares one handle and one
/// lock, so records written from this process never interleave.
///
/// Keyed by the canonical parent directory joined with the file name as given,
/// so `a/b.log` and `./a/../a/b.log` match but a symlink or a hard link to the
/// same file does not. Entries are also revalidated against the file actually
/// on disk, so a rotated-away log does not keep receiving records.
fn open_files() -> &'static Mutex<HashMap<PathBuf, Weak<Mutex<File>>>> {
    static OPEN_FILES: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<File>>>>> = OnceLock::new();
    OPEN_FILES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether `handle` is still the file that `path` names.
///
/// A log rotation renames or unlinks the file out from under an open handle;
/// without this check a new logger would inherit the cached handle and keep
/// appending to the rotated-away file.
///
/// Only Unix exposes file identity on stable Rust. Elsewhere the check is
/// skipped, and a replaced file keeps its old handle until every logger holding
/// it is dropped.
#[cfg(unix)]
fn is_same_file(handle: &File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    match (handle.metadata(), std::fs::metadata(path)) {
        (Ok(open), Ok(named)) => open.dev() == named.dev() && open.ino() == named.ino(),
        // The path is gone, so whatever we hold open is no longer it.
        _ => false,
    }
}

#[cfg(not(unix))]
fn is_same_file(_handle: &File, _path: &Path) -> bool {
    true
}

/// Take a lock without caring about poisoning.
///
/// A panic elsewhere says nothing about the validity of an append-only file
/// handle, and a logger that stops recording after an unrelated panic is worse
/// than one that keeps going.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Report a logger problem without risking a panic of our own: a logger must not
/// take the program down, not even when stderr is gone.
fn warn(args: fmt::Arguments) {
    let stderr = io::stderr();
    let mut handle = stderr.lock();
    let _ = writeln!(handle, "loggers: {args}");
}

/// Echo a record to stdout, tolerating a closed or broken stdout.
fn echo(args: fmt::Arguments) {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(handle, "{args}");
}

/// Dispatches each record to every [`log::Log`] that claims it, falling back to a
/// single catch-all logger when no other logger does.
pub struct Logger {
    loggers: Vec<Box<dyn log::Log>>,
    fallback: Option<Box<dyn log::Log>>,
}

impl Logger {
    pub fn new() -> Logger {
        Logger {
            loggers: Vec::new(),
            fallback: None,
        }
    }

    /// add a logger to the dispatcher
    /// # Arguments
    /// * `logger` - The logger to add, usually a [`CustomLogger`]
    /// # Example
    /// ```
    /// # use crate::loggers::*;
    /// let mut logger = Logger::new();
    /// logger.add_logger(Box::new(CustomLogger::new("test", "tests/output/system.log")));
    /// ```
    pub fn add_logger(&mut self, logger: Box<dyn log::Log>) {
        self.loggers.push(logger);
    }

    /// forward records that no registered logger claimed to `fallback`
    ///
    /// The fallback is a plain [`log::Log`], so it applies its own filtering and
    /// may still reject what it is handed. Build it with
    /// [`CustomLogger::catch_all`]: a [`CustomLogger::new`] fallback accepts only
    /// its own target and therefore discards the records it was meant to catch.
    /// # Arguments
    /// * `fallback` - The fallback logger
    /// # Example
    /// ```
    /// # use crate::loggers::*;
    /// let mut logger = Logger::new();
    /// logger.set_fallback(Box::new(CustomLogger::catch_all("tests/output/fallback.log")));
    /// ```
    pub fn set_fallback(&mut self, fallback: Box<dyn log::Log>) {
        self.fallback = Some(fallback);
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.loggers.iter().any(|l| l.enabled(metadata))
            || self.fallback.as_ref().is_some_and(|f| f.enabled(metadata))
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
                // Ask before handing the record over. A `Log` implementation is
                // allowed to assume a facade filtered for it, and its answer can
                // change between calls.
                if fallback.enabled(record.metadata()) {
                    fallback.log(record);
                }
            }
        }
    }

    fn flush(&self) {
        for logger in &self.loggers {
            logger.flush();
        }
        if let Some(fallback) = &self.fallback {
            fallback.flush();
        }
    }
}

/// Appends records to a file as JSON Lines, and echoes them to stdout unless
/// [`with_stdout(false)`](CustomLogger::with_stdout) turns that off.
///
/// The file is opened in append mode and kept open for the lifetime of the
/// logger; existing contents are never truncated.
///
/// Loggers sharing a path share one handle and one lock, so their records cannot
/// interleave. Path identity is the canonical parent directory plus the file
/// name as given: a symlink, a hard link, or a case-variant name on a
/// case-insensitive filesystem resolves to a *separate* handle, and so does
/// another process appending to the same file. None of those are covered by the
/// no-interleaving guarantee.
pub struct CustomLogger {
    /// `None` accepts any target; `Some(t)` accepts only records targeting `t`.
    target: Option<String>,
    filepath: PathBuf,
    /// `None` once the file could not be opened; the logger then echoes only.
    file: Option<SharedFile>,
    /// Whether to also print each accepted record to stdout.
    echo: bool,
}

impl CustomLogger {
    /// Build a logger that only accepts records whose target is exactly `target`.
    /// # Arguments
    /// * `target` - The log target to accept
    /// * `filepath` - The file to append to; parent directories are created
    /// # Example
    /// ```
    /// # use crate::loggers::*;
    /// let logger = CustomLogger::new("test", "tests/output/system.log");
    /// ```
    pub fn new(target: &str, filepath: impl AsRef<Path>) -> CustomLogger {
        Self::build(Some(target.to_string()), filepath.as_ref())
    }

    /// Build a logger that accepts records with any target.
    ///
    /// This is what [`Logger::set_fallback`] expects.
    /// # Arguments
    /// * `filepath` - The file to append to; parent directories are created
    /// # Example
    /// ```
    /// # use crate::loggers::*;
    /// let logger = CustomLogger::catch_all("tests/output/fallback.log");
    /// ```
    pub fn catch_all(filepath: impl AsRef<Path>) -> CustomLogger {
        Self::build(None, filepath.as_ref())
    }

    /// Opens the log file, reporting failure on stderr instead of panicking —
    /// a logger that cannot write must not take the program down with it.
    fn build(target: Option<String>, filepath: &Path) -> CustomLogger {
        let file = match Self::open(filepath) {
            Ok(file) => Some(file),
            Err(e) => {
                warn(format_args!("cannot open {}: {e}", filepath.display()));
                None
            }
        };

        CustomLogger {
            target,
            filepath: filepath.to_path_buf(),
            file,
            echo: true,
        }
    }

    fn open(filepath: &Path) -> io::Result<SharedFile> {
        let key = Self::key(filepath)?;
        let mut open_files = lock(open_files());

        if let Some(shared) = open_files.get(&key).and_then(Weak::upgrade) {
            // Lock order is always registry then file, so this cannot deadlock
            // against a concurrent write.
            if is_same_file(&lock(&shared), &key) {
                return Ok(shared);
            }
            // The path names a different file now. Open it; loggers still
            // holding the old handle go on writing to the rotated-away file.
        }

        let file = OpenOptions::new().create(true).append(true).open(&key)?;
        let shared: SharedFile = Arc::new(Mutex::new(file));
        // Loggers come and go; drop the entries whose files are already closed.
        open_files.retain(|_, weak| weak.strong_count() > 0);
        open_files.insert(key, Arc::downgrade(&shared));
        Ok(shared)
    }

    /// An absolute path, so that `a/b.log` and `./a/../a/b.log` share a handle.
    /// Canonicalising the parent rather than the file itself works before the
    /// log file exists.
    fn key(filepath: &Path) -> io::Result<PathBuf> {
        let file_name = filepath.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a file path", filepath.display()),
            )
        })?;
        let parent = match filepath.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        };
        std::fs::create_dir_all(parent)?;
        Ok(parent.canonicalize()?.join(file_name))
    }

    /// Turn the stdout echo on or off. It is on by default.
    ///
    /// Every logger that accepts a record echoes it, so two loggers watching one
    /// target print the record twice. Silence the ones you do not want to read.
    /// # Example
    /// ```
    /// # use crate::loggers::*;
    /// // Writes tests/output/quiet.log without touching stdout.
    /// let logger = CustomLogger::new("test", "tests/output/quiet.log").with_stdout(false);
    /// ```
    pub fn with_stdout(mut self, echo: bool) -> CustomLogger {
        self.echo = echo;
        self
    }

    /// The file this logger appends to.
    pub fn filepath(&self) -> &Path {
        &self.filepath
    }
}

impl log::Log for CustomLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        match &self.target {
            Some(target) => metadata.target() == target,
            None => true,
        }
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        // One timestamp for both renderings, so they can never disagree.
        let timestamp = Local::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let level = record.level().as_str();
        let target = record.target();
        let message = record.args().to_string();

        // Built with serde_json so quotes, backslashes and newlines in the
        // message stay escaped and each line remains parseable JSON.
        let mut line = serde_json::json!({
            "severity": level,
            "timestamp": timestamp,
            "target": target,
            "message": message,
        })
        .to_string();
        line.push('\n');

        if let Some(file) = &self.file {
            // Report outside the guard, so a failing stderr cannot stall or
            // poison the lock every other logger on this file is waiting for.
            let result = lock(file).write_all(line.as_bytes());
            if let Err(e) = result {
                warn(format_args!(
                    "cannot write {}: {e}",
                    self.filepath.display()
                ));
            }
        }

        if self.echo {
            echo(format_args!("[{level}] {target} {timestamp} - {message}"));
        }
    }

    fn flush(&self) {
        if let Some(file) = &self.file {
            let result = lock(file).flush();
            if let Err(e) = result {
                warn(format_args!(
                    "cannot flush {}: {e}",
                    self.filepath.display()
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log;

    fn emit(logger: &CustomLogger, target: &str, message: &str) {
        logger.log(
            &log::Record::builder()
                .args(format_args!("{message}"))
                .level(log::Level::Info)
                .target(target)
                .build(),
        );
    }

    /// A poisoned lock must not stop the logger: an unrelated panic says nothing
    /// about whether an append-only file handle is still usable.
    #[test]
    fn writes_survive_a_poisoned_lock() {
        let path = Path::new("tests/output/poison.log");
        let _ = std::fs::create_dir_all("tests/output");
        let _ = std::fs::remove_file(path);

        let logger = CustomLogger::new("poison", path);
        let file = logger.file.clone().expect("log file should be open");

        // Poison the lock the logger writes through.
        let handle = std::thread::spawn(move || {
            let _guard = file.lock().unwrap();
            panic!("poisoning the lock on purpose");
        });
        assert!(handle.join().is_err(), "the helper thread should panic");

        logger.log(
            &log::Record::builder()
                .args(format_args!("after poisoning"))
                .level(log::Level::Info)
                .target("poison")
                .build(),
        );
        logger.flush();

        let written = std::fs::read_to_string(path).unwrap();
        assert!(
            written.contains("after poisoning"),
            "poisoned lock swallowed the record: {written:?}"
        );
    }

    /// Two loggers on the same path must share one handle, so that neither can
    /// interleave a half-written line into the other's record.
    #[test]
    fn loggers_on_the_same_path_share_one_handle() {
        let _ = std::fs::create_dir_all("tests/output");
        let path = Path::new("tests/output/shared_handle.log");
        let _ = std::fs::remove_file(path);

        let first = CustomLogger::new("a", path);
        let second = CustomLogger::new("b", "tests/output/../output/shared_handle.log");

        let a = first.file.as_ref().expect("first handle");
        let b = second.file.as_ref().expect("second handle");
        assert!(
            Arc::ptr_eq(a, b),
            "same log file resolved to two independent handles"
        );
    }

    /// A rotated log must not keep receiving records through a cached handle:
    /// once the path names a different file, a new logger opens the new file.
    #[test]
    fn a_rotated_file_gets_a_fresh_handle() {
        let _ = std::fs::create_dir_all("tests/output");
        let path = Path::new("tests/output/rotate.log");
        let rotated = Path::new("tests/output/rotate.log.1");
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(rotated);

        let before = CustomLogger::new("rot", path);
        emit(&before, "rot", "written before rotation");

        // What logrotate does: move the file aside, leaving the name free.
        std::fs::rename(path, rotated).unwrap();

        let after = CustomLogger::new("rot", path);
        assert!(
            !Arc::ptr_eq(before.file.as_ref().unwrap(), after.file.as_ref().unwrap()),
            "the new logger inherited the handle of the rotated-away file"
        );

        emit(&after, "rot", "written after rotation");

        let new_file = std::fs::read_to_string(path).unwrap();
        assert!(new_file.contains("written after rotation"));
        assert!(
            !new_file.contains("written before rotation"),
            "the pre-rotation record leaked into the new file"
        );
        // The old handle still points at the file it opened, as an open fd does.
        assert!(
            std::fs::read_to_string(rotated)
                .unwrap()
                .contains("written before rotation")
        );
    }

    /// Documents a known limit: aliases of one file are *not* unified, so they
    /// get separate locks and fall outside the no-interleaving guarantee.
    #[cfg(unix)]
    #[test]
    fn a_symlink_alias_is_not_unified_with_its_target() {
        let _ = std::fs::create_dir_all("tests/output");
        let target = Path::new("tests/output/symlink_target.log");
        let alias = Path::new("tests/output/symlink_alias.log");
        let _ = std::fs::remove_file(target);
        let _ = std::fs::remove_file(alias);
        std::fs::File::create(target).unwrap();
        std::os::unix::fs::symlink("symlink_target.log", alias).unwrap();

        let direct = CustomLogger::new("a", target);
        let through_link = CustomLogger::new("b", alias);

        assert!(
            !Arc::ptr_eq(
                direct.file.as_ref().unwrap(),
                through_link.file.as_ref().unwrap()
            ),
            "symlink unification is not implemented; update the docs if it is"
        );
    }

    /// The registry must not grow forever as loggers come and go.
    #[test]
    fn dead_registry_entries_are_pruned() {
        let _ = std::fs::create_dir_all("tests/output");
        let transient = Path::new("tests/output/transient.log");
        let key = CustomLogger::key(transient).unwrap();

        drop(CustomLogger::new("gone", transient));
        // Opening any other file triggers the sweep.
        let _keeper = CustomLogger::new("keeper", "tests/output/keeper.log");

        assert!(
            !lock(open_files()).contains_key(&key),
            "a closed log file left a dangling registry entry"
        );
    }

    /// The echo is part of the published behaviour, so pin the default and both
    /// directions of the toggle. `log()` honouring the flag is checked by
    /// `disabling_the_echo_leaves_the_file_output_alone`.
    #[test]
    fn the_stdout_echo_is_on_by_default() {
        let _ = std::fs::create_dir_all("tests/output");
        let logger = CustomLogger::new("echo", "tests/output/echo_default.log");
        assert!(logger.echo, "the stdout echo should default to on");
    }

    #[test]
    fn with_stdout_toggles_the_echo() {
        let _ = std::fs::create_dir_all("tests/output");
        let off = CustomLogger::new("echo", "tests/output/echo_toggle.log").with_stdout(false);
        assert!(!off.echo);

        let on = off.with_stdout(true);
        assert!(on.echo, "the toggle should work in both directions");
    }

    /// Silencing stdout must not silence the log file — the two are independent.
    #[test]
    fn disabling_the_echo_leaves_the_file_output_alone() {
        let _ = std::fs::create_dir_all("tests/output");
        let path = Path::new("tests/output/echo_off.log");
        let _ = std::fs::remove_file(path);

        let logger = CustomLogger::new("quiet", path).with_stdout(false);
        emit(&logger, "quiet", "recorded but not printed");

        assert!(
            std::fs::read_to_string(path)
                .unwrap()
                .contains("recorded but not printed"),
            "turning the echo off also stopped the file output"
        );
    }

    #[test]
    fn distinct_paths_do_not_share_a_handle() {
        let _ = std::fs::create_dir_all("tests/output");
        let first = CustomLogger::new("a", "tests/output/distinct_a.log");
        let second = CustomLogger::new("b", "tests/output/distinct_b.log");

        assert!(!Arc::ptr_eq(
            first.file.as_ref().unwrap(),
            second.file.as_ref().unwrap()
        ));
    }
}
