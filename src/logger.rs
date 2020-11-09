use log::{self, Metadata, Record};

pub(crate) struct AlwaysLogger;

impl log::Log for AlwaysLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{}", record.args());
        }
    }

    fn flush(&self) {}
}

pub(crate) static LOGGER: AlwaysLogger = AlwaysLogger;

pub(crate) fn init_logger() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .unwrap();
}
