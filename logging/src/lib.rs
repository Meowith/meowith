use std::fmt::Debug;
use std::path::Path;

use log::{warn, LevelFilter};
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Config, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;

pub fn initialize_test_logging() {
    #[cfg(feature = "test_logging")]
    initialize_default(LevelFilter::Trace)
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
        .encoder(Box::new(PatternEncoder::new(
            "\x1b[94m{d(%Y-%m-%d %H:%M:%S.%6f)}\x1b[0m - {h({l})} - {m}{n}",
        )))
        .build();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .logger(Logger::builder().build("protocol", level))
        .logger(Logger::builder().build("node", level))
        .logger(Logger::builder().build("node_lib", level))
        .logger(Logger::builder().build("dashboard", level))
        .logger(Logger::builder().build("dashboard_lib", level))
        .logger(Logger::builder().build("controller", level))
        .logger(Logger::builder().build("controller_lib", level))
        .logger(Logger::builder().build("data", level))
        .logger(Logger::builder().build("tests", level))
        .logger(Logger::builder().build("commons", level))
        .logger(Logger::builder().build("auth_framework", level))
        .build(Root::builder().appender("stdout").build(LevelFilter::Info))
        .unwrap();

    let _ = log4rs::init_config(config);
}

pub fn log_err<T: Debug, E: Debug>(quote: &str, res: Result<T, E>) {
    if res.is_err() {
        let res = res.unwrap_err();
        warn!("{quote} - {res:?}");
    }
}
