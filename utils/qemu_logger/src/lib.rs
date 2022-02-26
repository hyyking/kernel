#![no_std]

use core::fmt::Write;

use kcore::{klazy, sync::SpinMutex};
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

/// # Errors
///
/// Forwards [`log::set_logger`] error
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
        let module = record.module_path_static().unwrap_or("");
        let line = record.line().unwrap_or(0);
        let args = record.args();
        match record.level() {
            log::Level::Trace => _qprint(format_args!(
                "\u{001b}[38;2;128;128;128;2m[{}][{}:{}] > {}\u{001b}[0m\n",
                level(record.level()),
                module,
                line,
                args
            )),
            log::Level::Debug => _qprint(format_args!(
                "\u{001b}[4;1m[{}][{}:{}]\u{001b}[0m > {}\n",
                level(record.level()),
                module,
                line,
                args
            )),
            _ => _qprint(format_args!(
                "[{}][{}:{}] > {}\n",
                level(record.level()),
                module,
                line,
                args
            )),
        }
    }

    fn flush(&self) {}
}

const fn level(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "\u{001b}[31;1mERROR\u{001b}[0m",
        log::Level::Warn => "\u{001b}[33;1mWARN\u{001b}[0m",
        log::Level::Info => "\u{001b}[34;1mINFO\u{001b}[0m",
        log::Level::Debug => "DEBUG",
        log::Level::Trace => "TRACE",
    }
}
