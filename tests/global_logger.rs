//! End-to-end check through the real `log` macros. `set_boxed_logger` succeeds
//! once per process, so this file holds exactly one test.

mod common;

use common::{lines, out, read};
use log::{debug, info};
use loggers::{CustomLogger, Logger};

#[test]
fn macros_route_through_the_global_logger() {
    let app = out("global_app.log");
    let fallback = out("global_fallback.log");

    let mut logger = Logger::new();
    logger.add_logger(Box::new(CustomLogger::new("test", &app)));
    logger.set_fallback(Box::new(CustomLogger::catch_all(&fallback)));

    log::set_boxed_logger(Box::new(logger)).expect("Failed to set logger");
    log::set_max_level(log::LevelFilter::Trace);

    info!(target: "test", "Hello, world!");
    debug!("Default");

    let claimed = lines(&app);
    assert_eq!(claimed.len(), 1, "expected exactly one targeted record");
    assert_eq!(claimed[0]["severity"], "INFO");
    assert_eq!(claimed[0]["target"], "test");
    assert_eq!(claimed[0]["message"], "Hello, world!");

    let caught = lines(&fallback);
    assert_eq!(
        caught.len(),
        1,
        "the untargeted record never reached the fallback"
    );
    assert_eq!(caught[0]["severity"], "DEBUG");
    assert_eq!(caught[0]["message"], "Default");
    assert_eq!(caught[0]["target"], "global_logger");

    assert!(!read(&app).contains("Default"));
}
