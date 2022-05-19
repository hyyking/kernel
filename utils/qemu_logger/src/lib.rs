#![no_std]

use core::{
    fmt::Write,
    sync::atomic::{AtomicU64, Ordering},
};

use kcore::{klazy, sync::SpinMutex};
use kio::{
    codec::{ChainedCodec, FramedWrite, Sink},
    cursor::Cursor,
};

use mais::CobsCodec;
use protocols::log::{Level, LogPacket, Message, Span};
use serialuart16550::SerialPort;

use rkyv::{
    ser::{
        serializers::{BufferScratch, BufferSerializer, CompositeSerializer, ScratchTracker},
        Serializer,
    },
    AlignedBytes,
};
use tracing_core::{
    span::{Attributes, Current, Id, Record},
    Event, Metadata,
};

#[macro_export]
macro_rules! dbg {
    ($arg:expr) => {{
        ::tracing::debug!("{} = {:#?}", stringify!($arg), $arg);
        $arg
    }};
}

const BUFFER_SIZE: usize = 512;

klazy! {
    // SAFETY: we are the only one accessing this port on initialization
    #[link_section = ".logger"]
    pub ref static DRIVER: SpinMutex<FramedWrite<AlignedBytes<BUFFER_SIZE>, SerialPort, ChainedCodec<AlignedBytes<BUFFER_SIZE>, LogEncoder,CobsCodec>>> = unsafe {
        let mut port = SerialPort::new(0x3f8);
        port.init();
        SpinMutex::new(
            FramedWrite::new(AlignedBytes([0; BUFFER_SIZE]), port, ChainedCodec::new(AlignedBytes([0; BUFFER_SIZE]), LogEncoder::new(), CobsCodec))
        )
    };
}

struct Logger;
static LOGGER: Logger = Logger;

static SPANS: AtomicU64 = AtomicU64::new(1);
static CURRENT_SPAN: AtomicU64 = AtomicU64::new(0);

// TODO: this probably doesn't work very well and should be a stack of metadata which is poped on span exit
static mut CURRENT_METADATA: *const Metadata = core::ptr::null();

/// # Errors
///
/// Forwards [`tracing::dispatch::set_global_default`] error
pub fn init() -> Result<(), tracing_core::dispatch::SetGlobalDefaultError> {
    tracing_core::dispatch::set_global_default(tracing_core::Dispatch::from_static(&LOGGER))?;
    Ok(())
}

pub struct LogEncoder {
    scratch_buffer: AlignedBytes<BUFFER_SIZE>,
}

impl LogEncoder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scratch_buffer: AlignedBytes([0u8; BUFFER_SIZE]),
        }
    }
}

pub static MAX_SCRATCH: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

impl kio::codec::Encoder<LogPacket<'_>> for LogEncoder {
    type Error = kio::Error;

    fn encode<T>(&mut self, item: LogPacket<'_>, dst: T) -> Result<usize, Self::Error>
    where
        T: AsMut<[u8]>,
    {
        let mut buffer = CompositeSerializer::new(
            BufferSerializer::new(dst),
            ScratchTracker::new(BufferScratch::new(&mut self.scratch_buffer[..])),
            rkyv::Infallible,
        );

        let n = buffer
            .serialize_unsized_value(&item)
            .map_err(|_| kio::Error {})?;

        let (_, scratch, _) = buffer.into_components();

        MAX_SCRATCH.store(
            scratch.max_bytes_allocated(),
            core::sync::atomic::Ordering::Relaxed,
        );

        Ok(n + 4)
    }
}

fn _qprint_encode(message: LogPacket<'_>) {
    DRIVER.lock().send(message).unwrap();
}

struct DebugArgs<'a>(Cursor<'a>);

impl<'a> From<Cursor<'a>> for DebugArgs<'a> {
    fn from(c: Cursor<'a>) -> Self {
        Self(c)
    }
}

impl tracing_core::field::Visit for DebugArgs<'_> {
    fn record_debug(&mut self, field: &tracing_core::Field, value: &dyn core::fmt::Debug) {
        self.0
            .write_fmt(format_args!("{} = {:?}", field.name(), value))
            .unwrap();
    }
}

impl tracing_core::Collect for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn new_span(&self, attr: &Attributes<'_>) -> Id {
        // NOTE: this is note thread safe, metadata is allways static so the pointer is valid
        unsafe {
            CURRENT_METADATA = attr.metadata();
        }
        let id = Id::from_u64(SPANS.fetch_add(1, Ordering::Relaxed));

        let span = Span {
            id: id.into_u64(),
            target: attr.metadata().target(),
        };

        libx64::without_interrupts(|| _qprint_encode(LogPacket::NewSpan(span)));
        id
    }

    #[inline]
    fn record(&self, _span: &Id, _values: &Record<'_>) {
        // TODO
    }

    #[inline]
    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut buffer = [0u8; BUFFER_SIZE];
        let mut args = DebugArgs::from(Cursor::new(&mut buffer));
        event.record(&mut args);
        let message = unsafe { core::str::from_utf8_unchecked(args.0.buffer()) };

        let metadata = event.metadata();
        let log = Message {
            level: level_from_tracing(*metadata.level()),
            line: metadata.line().unwrap_or(0),
            path: metadata.module_path().unwrap_or("notfound"),
            message,
        };

        libx64::without_interrupts(|| _qprint_encode(LogPacket::Message(log)));
    }

    #[inline]
    fn enter(&self, span: &Id) {
        CURRENT_SPAN.store(span.into_u64(), Ordering::Relaxed);
        libx64::without_interrupts(|| _qprint_encode(LogPacket::EnterSpan(span.into_u64())));
    }

    #[inline]
    fn exit(&self, span: &Id) {
        CURRENT_SPAN.store(0, Ordering::Relaxed);
        libx64::without_interrupts(|| _qprint_encode(LogPacket::ExitSpan(span.into_u64())));
    }

    fn current_span(&self) -> Current {
        unsafe {
            Current::new(
                Id::from_u64(CURRENT_SPAN.load(Ordering::Relaxed)),
                &*CURRENT_METADATA,
            )
        }
    }
}

const fn level_from_tracing(level: tracing_core::Level) -> Level {
    match level {
        tracing_core::Level::ERROR => Level::Error,
        tracing_core::Level::WARN => Level::Warn,
        tracing_core::Level::INFO => Level::Info,
        tracing_core::Level::DEBUG => Level::Debug,
        tracing_core::Level::TRACE => Level::Trace,
    }
}
