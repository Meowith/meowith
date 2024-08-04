use std::path::Path;

use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

pub fn initialize_test_logging() {
    #[cfg(feature = "test_logging")]
    initialize_default(LevelFilter::Debug)
}

/// Looks for the config in the provided path.
/// If not found, initialize the default.
pub fn initialize_logging(config_path: Option<&Path>) {
    if let Some(path) = config_path {
        if path.exists() {
            log4rs::init_file(path, Default::default()).expect("Logging init failed");
        } else {
            initialize_default(LevelFilter::Info)
        }
    } else {
        initialize_default(LevelFilter::Info)
    }
}

fn initialize_default(level: LevelFilter) {
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l} - {m}{n}")))
        .build();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(level))
        .unwrap();

    let _ = log4rs::init_config(config);
}
