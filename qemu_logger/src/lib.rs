#![no_std]

use core::fmt::Write;

use kcore::{klazy, sync::mutex::SpinMutex};
use serialuart16550::SerialPort;

klazy! {
    // SAFETY: we are the only one accessing this port
    ref static DRIVER: SpinMutex<SerialPort> = unsafe {
        let mut port = SerialPort::new(0x3F8);
        port.init();
        SpinMutex::new(port)
    };
}

fn _qprint(args: core::fmt::Arguments) {
    DRIVER.lock().write_fmt(args).expect("qprint");
}

#[macro_export]
macro_rules! dbg {
    ($arg:expr) => {{
        ::log::debug!("{} = {:#?}", stringify!($arg), $arg);
        $arg
    }};
}

pub fn init() -> Result<(), log::SetLoggerError> {
    log::set_logger(&LOGGER)?;
    log::set_max_level(log::LevelFilter::Trace);
    Ok(())
}

struct Logger;
static LOGGER: Logger = Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace
    }

    fn log(&self, record: &log::Record) {
        _qprint(format_args!(
            "[{}][{}:{}] > {}\n",
            colored_level(record.level()),
            record.module_path_static().unwrap_or(""),
            record.line().unwrap_or(0),
            record.args(),
        ));
    }

    fn flush(&self) {}
}

fn colored_level<'a>(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "ERROR",
        log::Level::Warn => "WARN ",
        log::Level::Info => "INFO ",
        log::Level::Debug => "DEBUG",
        log::Level::Trace => "TRACE",
    }
}
