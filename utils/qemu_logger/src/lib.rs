#![no_std]

use kcore::{klazy, sync::SpinMutex};
use protocols::log::{Level, LogHeader, LogMessage, SIZE_PAD};
use serialuart16550::SerialPort;

use rkyv::{
    ser::{
        serializers::{BufferScratch, BufferSerializer, CompositeSerializer, ScratchTracker},
        Serializer,
    },
    AlignedBytes,
};

struct PortSerializer {
    port: SerialPort,
    pos: usize,
}

struct RkyvLogger {
    ser: PortSerializer,
    buffer: AlignedBytes<512>,
    scratch: AlignedBytes<512>,
}

impl rkyv::Fallible for PortSerializer {
    type Error = ();
}

impl Serializer for PortSerializer {
    fn pos(&self) -> usize {
        self.pos
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        bytes.iter().copied().for_each(|b| self.port.send(b));
        self.pos += bytes.len();
        Ok(())
    }
}

klazy! {
    // SAFETY: we are the only one accessing this port
    ref static DRIVER: SpinMutex<RkyvLogger> = unsafe {
        let mut port = SerialPort::new(0x3f8);
        port.init();
        SpinMutex::new(RkyvLogger {
            ser: PortSerializer { port, pos: 0 },
            buffer: AlignedBytes([0; 512]),
            scratch: AlignedBytes([0; 512])
        })
    };
}

pub static MAX_SCRATCH: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

fn _qprint_encode(message: LogMessage<'_>) {
    let mut lock = DRIVER.lock();
    let logger = &mut *lock;

    let mut buffer = CompositeSerializer::new(
        BufferSerializer::new(&mut logger.buffer[..]),
        ScratchTracker::new(BufferScratch::new(&mut logger.scratch[..])),
        rkyv::Infallible,
    );

    let n = buffer.serialize_unsized_value(&message).unwrap() + SIZE_PAD; //.expect("lol");

    let (buffer, scratch, _) = buffer.into_components();

    logger.ser.serialize_value(&LogHeader { size: n }).unwrap();
    logger.ser.write(&buffer.into_inner()[..n]).unwrap();

    MAX_SCRATCH.store(
        scratch.max_bytes_allocated(),
        core::sync::atomic::Ordering::Relaxed,
    );
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
        let mut message = [0u8; 512];

        let mut cursor = kcore::io::Cursor::new(&mut message[..]);

        if let Err(_) = core::fmt::write(&mut cursor, format_args!("{}", record.args())) {
            _qprint_encode(LogMessage {
                level: Level::Error,
                line: cursor.buffer().len() as u32,
                path: file!(),
                message: "oom formating log",
            });
        }

        let message = unsafe { core::str::from_utf8_unchecked(cursor.buffer()) };

        let log = LogMessage {
            level: level_from_log(record.level()),
            line: record.line().unwrap_or(0),
            path: record.module_path_static().unwrap_or("notfound"),
            message,
        };

        libx64::without_interrupts(|| _qprint_encode(log));
    }

    fn flush(&self) {}
}

const fn level_from_log(level: log::Level) -> Level {
    match level {
        log::Level::Error => Level::Error,
        log::Level::Warn => Level::Warn,
        log::Level::Info => Level::Info,
        log::Level::Debug => Level::Debug,
        log::Level::Trace => Level::Trace,
    }
}
