[English](behaviour.md) | [日本語](behaviour.ja.md)

# Behaviour and guarantees

This page states what the crate promises, and — just as importantly — where each
promise stops.

## Routing

Every logger whose target matches receives the record. If none match, the
record is offered to the fallback, which may still decline it: `set_fallback`
takes any `log::Log`, and the dispatcher asks its `enabled` before handing the
record over.

## Stdout echo

Every logger that accepts a record echoes it, so a record claimed by two
loggers is printed twice. `CustomLogger::with_stdout(false)` turns the echo off
for that logger without affecting its file output.

## Append-only

Log files are opened in append mode and the handle is held for the lifetime of
the logger. Existing contents are never truncated, and a logger is never the
reason an earlier run's log disappeared.

## Escaping

Lines are built with `serde_json`, so quotes, backslashes and newlines in a
message stay escaped. Every line remains parseable on its own, which is what
makes the JSON Lines format usable.

## Line-atomic within a process

Loggers pointed at the same path share one file handle and one lock, so their
records cannot interleave.

Path identity is the canonical parent directory plus the file name as given.
The following each resolve to a *separate* handle and fall outside the
guarantee:

- a symbolic link to the same file
- a hard link to the same file
- a case-variant name on a case-insensitive filesystem
- another *process* appending to the same file

## Rotation-aware

A cached handle is revalidated against the file the path currently names, so a
logger created after `logrotate` moved the file aside opens the new file rather
than inheriting the old one.

Loggers that were already open keep writing to the file they opened, as any
held file descriptor does. File identity is only available on Unix; elsewhere a
replaced file keeps its handle until every logger holding it is dropped.

## I/O errors do not panic

A log file that cannot be opened, written or flushed is reported on stderr, and
the logger degrades to the stdout echo rather than aborting the program. A
poisoned lock is recovered rather than disabling output for good.

This is not a claim that the crate is panic-free in general: a record whose own
`Display` implementation panics will still panic while being formatted, before
this crate sees it.
