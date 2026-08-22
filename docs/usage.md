[English](usage.md) | [日本語](usage.ja.md)

# Usage

## Installation

```bash
cargo add loggers
```

Requires Rust 1.85 or later (edition 2024).

## Quick start

```rust
use log::{debug, info};
use loggers::*;

let mut logger = Logger::new();

// Records targeting "test" go to system.log.
logger.add_logger(Box::new(CustomLogger::new("test", "tests/output/system.log")));

// Everything else goes to fallback.log.
logger.set_fallback(Box::new(CustomLogger::catch_all("tests/output/fallback.log")));

log::set_boxed_logger(Box::new(logger)).expect("Failed to set logger");
log::set_max_level(log::LevelFilter::Trace);

info!(target: "test", "Hello, world!");
debug!("Default");
```

## Output format

Each accepted record is appended to the log file as one line of JSON — the
JSON Lines format, so the file can be read back line by line.

```json
{"severity":"INFO","timestamp":"2026-08-22T15:30:00.000+09:00","target":"test","message":"Hello, world!"}
```

| Field | Contents |
| --- | --- |
| `severity` | The record's level: `ERROR`, `WARN`, `INFO`, `DEBUG` or `TRACE` |
| `timestamp` | Local time, RFC 3339 with millisecond precision |
| `target` | The record's own target, not the logger's name |
| `message` | The formatted message |

The same record is also echoed to stdout in a human-readable form:

```text
[INFO] test 2026-08-22T15:30:00.000+09:00 - Hello, world!
```

## Routing by target

`Logger` is a dispatcher. Add one `CustomLogger` per target you want to
separate, and each record goes to every logger whose target matches it.

```rust
use loggers::*;

let mut logger = Logger::new();
logger.add_logger(Box::new(CustomLogger::new("api", "logs/api.log")));
logger.add_logger(Box::new(CustomLogger::new("db", "logs/db.log")));
```

A record with target `"api"` lands in `logs/api.log` only. Two loggers may
claim the same target, in which case both record it.

## The fallback

A record that no registered logger claims is offered to the fallback. Build
the fallback with `CustomLogger::catch_all`, which accepts any target:

```rust
use loggers::*;

let mut logger = Logger::new();
logger.add_logger(Box::new(CustomLogger::new("api", "logs/api.log")));
logger.set_fallback(Box::new(CustomLogger::catch_all("logs/everything-else.log")));
```

`CustomLogger::new` builds a *target-filtered* logger, so using it as a
fallback makes the fallback reject the very records it is meant to catch.
`set_fallback` accepts any `log::Log`, so this still compiles — it just does
not do what a fallback is for.

Because the logged `target` is the record's own target, a catch-all fallback
still tells you where each record came from.

## Turning off the stdout echo

Every logger that accepts a record echoes it to stdout, so a record claimed by
two loggers is printed twice. Silence the echo per logger when you want the
file only:

```rust
use loggers::*;

let quiet = CustomLogger::new("api", "logs/api.log").with_stdout(false);
```

The file output is unaffected.

## Using a single logger directly

`CustomLogger` is a `log::Log` in its own right, so it can be installed without
a `Logger` dispatcher when one file is all you need:

```rust
use loggers::*;

log::set_boxed_logger(Box::new(CustomLogger::catch_all("logs/app.log")))
    .expect("Failed to set logger");
log::set_max_level(log::LevelFilter::Info);
```

## API reference

The full API documentation is published on
[docs.rs](https://docs.rs/loggers).
