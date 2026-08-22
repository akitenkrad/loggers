[English](migration.md) | [日本語](migration.ja.md)

# Migrating from 0.1.x

0.2.0 changes behaviour that 0.1.x got wrong, so the upgrade is not a drop-in
one. The full list of changes is in the
[changelog](../CHANGELOG.md).

## The fallback needs `catch_all`

```rust
# use loggers::*;
# let mut logger = Logger::new();
// 0.1.x
logger.set_fallback(Box::new(CustomLogger::new("default", "logs/fallback.log")));

// 0.2.0
logger.set_fallback(Box::new(CustomLogger::catch_all("logs/fallback.log")));
```

In 0.1.x the fallback applied its own target filter to whatever it was handed,
so it recorded only the records whose target happened to equal the fallback's
own name and silently dropped the rest — which is to say it did not work as a
catch-all. `CustomLogger::catch_all` accepts any target, which is what a
fallback needs.

`set_fallback` still takes any `log::Log`, so the old spelling continues to
compile. It just keeps behaving as it did in 0.1.x.

## Log files are no longer truncated

`CustomLogger::new` used to call `File::create`, which truncates. It now opens
in append mode. If you relied on the old behaviour to start each run with an
empty file, delete the file yourself before constructing the logger.

## The `target` field changed meaning

The logged `target` now holds the record's own target rather than the logger's
name. The two are identical for a target-filtered logger, and differ only for a
catch-all fallback — where the record's target is the more useful of the two.

If you parse the log files and match on `target`, check that nothing depended
on the fallback's records all carrying the fallback's name.

## Output is now valid JSON

0.1.x assembled each line with `format!`, so a quote, backslash or newline in a
message produced a line that would not parse. Those lines are now escaped
properly. A parser that worked around the old breakage can be simplified.

## New in 0.2.0

- `CustomLogger::catch_all(path)` — a fallback that accepts any target.
- `CustomLogger::with_stdout(false)` — file-only output.
- `CustomLogger::filepath()` — the path a logger appends to.
- `CustomLogger::new` and `catch_all` take `impl AsRef<Path>`, so a `PathBuf`
  works as well as a `&str`.
