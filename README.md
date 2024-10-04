# loggers

## Loggers

### Installation
```bash
cargo add loggers
```

## Examples
```rust
use loggers::*;
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
```

License: Apache-2.0
