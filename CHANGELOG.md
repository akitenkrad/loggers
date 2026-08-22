# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0]

### Fixed

- **The fallback logger did not work as a catch-all.** `CustomLogger::log`
  re-checked the record against its own target, so a fallback kept only the
  records whose target happened to equal the fallback's own name and dropped
  everything else it was handed. Fallbacks are now built with
  `CustomLogger::catch_all`, which accepts any target.
- **Log lines were not valid JSON.** The line was assembled with `format!`, so a
  quote, backslash or newline in a message produced unparseable output. Lines are
  now built with `serde_json`.
- **Writes could be silently truncated.** `Write::write` was called without
  checking the returned length (`clippy::unused_io_amount`); a partial write
  dropped part of a record. Replaced with a single `write_all` per record.
- **`CustomLogger::new` wiped existing logs.** It called `File::create`, which
  truncates. Files are now opened in append mode.
- **The crate-level doctest did not compile**, so `cargo test` failed and the
  example on docs.rs could not be copied as-is.
- **Constructing a logger could panic** on a failed directory or file creation.
  Failures are now reported on stderr and the logger degrades to stdout.

- **Concurrent writers could shred each other's lines.** Each logger held its
  own file handle and lock, so two loggers on one path could interleave partial
  writes. Loggers now share one handle and one lock per file.
- **A poisoned lock silently disabled file output forever.** `lock()` errors
  were discarded, so after any unrelated panic every write and flush became a
  no-op. Poisoning is now recovered from, and diagnostics are emitted outside
  the guard so a failing stderr cannot poison the lock in turn.
- **`flush()` errors were discarded.** They are now reported like write errors.
- **A rotated log kept receiving records.** Sharing one handle per path meant a
  logger created after `logrotate` moved the file aside inherited the handle of
  the rotated-away file. Cached handles are now revalidated against the file the
  path currently names (Unix only; see the README for the exact boundary).
- **`println!`/`eprintln!` could panic** on a closed stdout or stderr, which
  would have taken down the program the error handling was meant to protect.
  Both now write through a locked handle and ignore the result.

### Changed

- `Logger::flush` now flushes every registered logger and the fallback, instead
  of doing nothing.
- `Logger::enabled` now reflects the registered loggers instead of always
  returning `true`.
- The logged `target` field is the record's own target, not the logger's name,
  so a catch-all fallback reports where each record came from.
- The log file is opened once and held for the logger's lifetime, rather than
  reopened for every record.
- `CustomLogger::new` and `catch_all` take `impl AsRef<Path>` instead of `&str`.
- Edition 2021 → 2024 (`rust-version = "1.85"`).
- Dependencies raised to their current releases: `chrono` 0.4.45, `log` 0.4.33,
  `serde_json` 1.0.151.
- `Logger::log` now consults the fallback's `enabled` before handing a record
  over, instead of relying on the fallback to filter itself. A `Log`
  implementation may assume a facade filtered for it.
- `Logger::enabled` now consults the fallback's own `enabled`, instead of
  reporting `true` merely because a fallback is registered. A target-filtered
  fallback — the 0.1.x spelling, which still compiles — is therefore visible
  through `log_enabled!` rather than silently swallowing records.
- README no longer claims the crate is panic-free; it states what is actually
  guaranteed (I/O errors and lock poisoning do not panic) and what is not.

### Added

- `CustomLogger::catch_all` for building a fallback logger.
- `CustomLogger::with_stdout` to silence the stdout echo per logger. Every
  logger that accepts a record echoes it, so a record claimed by two loggers is
  printed twice; this is how you avoid that, and how you get file-only output.
- `CustomLogger::filepath` accessor.
- `Default` implementation for `Logger`.
- A regression test suite covering routing, fallback delivery, JSON escaping,
  append semantics and I/O failure.
- GitHub Actions CI running `fmt`, `clippy -D warnings`, `test` and `doc`, plus
  a second job that runs the test suite on the declared MSRV.
- Tests for concurrent writes, shared file handles, lock poisoning, log
  rotation, symlink aliasing, registry pruning and unopenable log files.

## [0.1.1]

- Initial release.
