mod common;

use common::{emit, lines, out, read};
use log::{Level, Log};
use loggers::{CustomLogger, Logger};

/// Regression: the fallback used to re-check the record against its own target
/// and silently drop everything it was supposed to catch.
#[test]
fn fallback_receives_unclaimed_records() {
    let app = out("routing_app.log");
    let fallback = out("routing_fallback.log");

    let mut logger = Logger::new();
    logger.add_logger(Box::new(CustomLogger::new("app", &app)));
    logger.set_fallback(Box::new(CustomLogger::catch_all(&fallback)));

    emit(&logger, "somewhere::else", Level::Debug, "unclaimed");

    assert!(
        read(&app).is_empty(),
        "record leaked into the target logger"
    );
    let caught = lines(&fallback);
    assert_eq!(caught.len(), 1);
    assert_eq!(caught[0]["message"], "unclaimed");
    // The fallback records the record's own target, not the fallback's name.
    assert_eq!(caught[0]["target"], "somewhere::else");
    assert_eq!(caught[0]["severity"], "DEBUG");
}

#[test]
fn matching_target_bypasses_the_fallback() {
    let app = out("matching_app.log");
    let fallback = out("matching_fallback.log");

    let mut logger = Logger::new();
    logger.add_logger(Box::new(CustomLogger::new("app", &app)));
    logger.set_fallback(Box::new(CustomLogger::catch_all(&fallback)));

    emit(&logger, "app", Level::Info, "claimed");

    let claimed = lines(&app);
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0]["message"], "claimed");
    assert!(read(&fallback).is_empty(), "fallback ran despite a match");
}

#[test]
fn records_without_a_fallback_are_dropped() {
    let app = out("nofallback_app.log");

    let mut logger = Logger::new();
    logger.add_logger(Box::new(CustomLogger::new("app", &app)));

    emit(&logger, "other", Level::Warn, "nobody wants this");

    assert!(read(&app).is_empty());
    assert!(!logger.enabled(&log::Metadata::builder().target("other").build()));
}

/// Regression: the JSON line was assembled with `format!`, so any quote,
/// backslash or newline in the message produced unparseable output.
#[test]
fn special_characters_stay_valid_json() {
    let path = out("escaping.log");
    let logger = CustomLogger::new("esc", &path);

    let nasty = "quote \" backslash \\ newline \n tab \t unicode ✓";
    emit(&logger, "esc", Level::Error, nasty);

    let entries = lines(&path);
    assert_eq!(entries.len(), 1, "an embedded newline split the record");
    assert_eq!(entries[0]["message"], nasty);
    assert_eq!(entries[0]["severity"], "ERROR");
}

/// Regression: the constructor used `File::create`, wiping an existing log.
#[test]
fn existing_log_is_appended_to_not_truncated() {
    let path = out("append.log");

    {
        let logger = CustomLogger::new("app", &path);
        emit(&logger, "app", Level::Info, "first run");
    }
    {
        let logger = CustomLogger::new("app", &path);
        emit(&logger, "app", Level::Info, "second run");
    }

    let entries = lines(&path);
    assert_eq!(
        entries.len(),
        2,
        "the second logger truncated the first's log"
    );
    assert_eq!(entries[0]["message"], "first run");
    assert_eq!(entries[1]["message"], "second run");
}

/// Two loggers may claim the same record; both should write it.
#[test]
fn every_matching_logger_receives_the_record() {
    let first = out("fanout_first.log");
    let second = out("fanout_second.log");

    let mut logger = Logger::new();
    logger.add_logger(Box::new(CustomLogger::new("app", &first)));
    logger.add_logger(Box::new(CustomLogger::new("app", &second)));

    emit(&logger, "app", Level::Info, "fan out");

    assert_eq!(lines(&first).len(), 1);
    assert_eq!(lines(&second).len(), 1);
}

/// Regression for the 0.1.x misuse: `set_fallback` takes a plain `log::Log`, so
/// a target-filtered fallback still compiles and still rejects everything. This
/// pins the documented behaviour that `catch_all` is what a fallback needs.
#[test]
fn a_target_filtered_fallback_still_rejects_records() {
    let app = out("misuse_app.log");
    let fallback = out("misuse_fallback.log");

    let mut logger = Logger::new();
    logger.add_logger(Box::new(CustomLogger::new("app", &app)));
    // The 0.1.x spelling. Kept compiling, so it must stay observably wrong.
    logger.set_fallback(Box::new(CustomLogger::new("default", &fallback)));

    emit(&logger, "somewhere::else", Level::Debug, "unclaimed");

    assert!(read(&app).is_empty());
    assert!(
        read(&fallback).is_empty(),
        "a filtered fallback unexpectedly accepted a foreign target"
    );
    // ...and `enabled` reports it, rather than claiming the record is covered.
    assert!(!logger.enabled(&log::Metadata::builder().target("somewhere::else").build()));
}

#[test]
fn a_catch_all_fallback_reports_itself_as_enabled() {
    let fallback = out("enabled_fallback.log");

    let mut logger = Logger::new();
    logger.set_fallback(Box::new(CustomLogger::catch_all(&fallback)));

    assert!(logger.enabled(&log::Metadata::builder().target("anything").build()));
}

/// Concurrent writers must not shred each other's lines.
#[test]
fn concurrent_writes_stay_line_atomic() {
    use std::sync::Arc;

    let path = out("concurrent.log");
    let logger = Arc::new(CustomLogger::catch_all(&path).with_stdout(false));

    const THREADS: usize = 8;
    const PER_THREAD: usize = 250;

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let logger = Arc::clone(&logger);
            std::thread::spawn(move || {
                for i in 0..PER_THREAD {
                    // A message long enough that a partial write would show up,
                    // and containing characters that must stay escaped.
                    emit(
                        logger.as_ref(),
                        "concurrent",
                        Level::Info,
                        &format!("{t}-{i} \"{}\"", "x".repeat(200)),
                    );
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    logger.flush();

    let entries = lines(&path);
    assert_eq!(entries.len(), THREADS * PER_THREAD, "records were lost");

    let mut seen: Vec<String> = entries
        .iter()
        .map(|e| e["message"].as_str().unwrap().to_string())
        .collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), THREADS * PER_THREAD, "records were duplicated");
}

/// Separate loggers on the same path share a lock, so they interleave cleanly.
///
/// This is a stress test, not the proof: a small `write_all` to a regular file
/// is atomic in practice on the usual platforms, so this would pass even
/// without the shared handle. The deterministic check lives in the unit test
/// `loggers_on_the_same_path_share_one_handle`, which fails outright if the
/// registry stops unifying handles.
#[test]
fn separate_loggers_on_one_path_stay_line_atomic() {
    let path = out("shared_path.log");

    const WRITERS: usize = 4;
    const PER_WRITER: usize = 200;

    let handles: Vec<_> = (0..WRITERS)
        .map(|w| {
            let path = path.clone();
            std::thread::spawn(move || {
                let logger = CustomLogger::new("shared", &path).with_stdout(false);
                for i in 0..PER_WRITER {
                    emit(
                        &logger,
                        "shared",
                        Level::Info,
                        &format!("{w}-{i} {}", "y".repeat(200)),
                    );
                }
                logger.flush();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    // `lines` panics on any line that is not valid JSON, which is what a torn
    // write would produce.
    assert_eq!(lines(&path).len(), WRITERS * PER_WRITER);
}

/// A logger whose file could never be opened must stay usable and inert.
#[test]
fn a_logger_that_could_not_open_its_file_stays_inert() {
    let blocker = out("blocker.log");
    std::fs::write(&blocker, b"").unwrap();

    let unusable = blocker.join("nested.log");
    let logger = CustomLogger::new("app", &unusable);

    emit(&logger, "app", Level::Info, "still alive");
    logger.flush();

    assert!(!unusable.exists(), "an unopenable path should stay absent");
    assert_eq!(
        logger.filepath(),
        unusable,
        "the logger should still report its configured path"
    );
    // The blocker file must not have been written through.
    assert!(read(&blocker).is_empty());
}

/// `/dev/full` accepts an open and fails every write with ENOSPC, which makes
/// the write-error path deterministic. Linux-only; macOS has no equivalent.
#[cfg(target_os = "linux")]
#[test]
fn a_failing_write_does_not_panic() {
    let logger = CustomLogger::new("full", "/dev/full");
    emit(&logger, "full", Level::Error, "this write cannot succeed");
    logger.flush();
}

/// A `Log` implementation is allowed to trust that a facade filtered for it.
/// The dispatcher must therefore ask the fallback before handing it a record,
/// rather than leaning on the fallback to re-check. A `CustomLogger` fallback
/// re-checks and so cannot detect this; a probe that does not, can.
#[test]
fn the_dispatcher_asks_the_fallback_before_handing_over() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Probe {
        accepts: bool,
        enabled_calls: AtomicUsize,
        log_calls: AtomicUsize,
    }

    impl log::Log for Probe {
        fn enabled(&self, _: &log::Metadata) -> bool {
            self.enabled_calls.fetch_add(1, Ordering::SeqCst);
            self.accepts
        }
        fn log(&self, _: &log::Record) {
            // Deliberately does NOT re-check `enabled`.
            self.log_calls.fetch_add(1, Ordering::SeqCst);
        }
        fn flush(&self) {}
    }

    struct Shared(Arc<Probe>);
    impl log::Log for Shared {
        fn enabled(&self, m: &log::Metadata) -> bool {
            self.0.enabled(m)
        }
        fn log(&self, r: &log::Record) {
            self.0.log(r)
        }
        fn flush(&self) {
            self.0.flush()
        }
    }

    let probe = Arc::new(Probe::default());
    let mut logger = Logger::new();
    logger.set_fallback(Box::new(Shared(Arc::clone(&probe))));

    emit(&logger, "anything", Level::Info, "unclaimed");

    assert!(
        probe.enabled_calls.load(Ordering::SeqCst) > 0,
        "the dispatcher never consulted the fallback"
    );
    assert_eq!(
        probe.log_calls.load(Ordering::SeqCst),
        0,
        "a record was pushed into a fallback that had declined it"
    );
}

/// The companion case: a fallback that accepts must still receive the record.
#[test]
fn an_accepting_fallback_still_receives_the_record() {
    let fallback = out("accepting_fallback.log");

    let mut logger = Logger::new();
    logger.set_fallback(Box::new(CustomLogger::catch_all(&fallback)));

    emit(&logger, "anything", Level::Info, "unclaimed");

    assert_eq!(lines(&fallback).len(), 1);
}
