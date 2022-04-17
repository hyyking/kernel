mod codec;

use std::io;

use protocols::log::{ArchivedLevel, ArchivedLogPacket};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let mut addr = std::env::args().skip(1);
    let addr = addr.next().ok_or(io::Error::new(
        io::ErrorKind::Other,
        "missing server address",
    ))?;

    let listener = TcpListener::bind(addr).await?;

    let (mut stream, _) = listener.accept().await?;

    let mut stdout = tokio::io::stdout();
    let mut codec = codec::LogDecoder::new();

    let mut bytes = bytes::BytesMut::new();
    while let Ok(n) = stream.read_buf(&mut bytes).await {
        if n == 0 {
            return Ok(());
        }

        let (n, message) = match codec.decode_ref(&mut bytes)? {
            Some((n, message)) => (n, message),
            None => continue,
        };

        let message = match Some(message) {
            Some(ArchivedLogPacket::Message(message)) => message,
            Some(ArchivedLogPacket::NewSpan(span)) => {
                dbg!("new", span.id, &*span.target);
                bytes.clear();
                continue
            },
            Some(ArchivedLogPacket::EnterSpan(span)) => {
                dbg!("enter", span);
                bytes.clear();
                continue
            },
            Some(ArchivedLogPacket::ExitSpan(span)) => {
                dbg!("exit", span);
                bytes.clear();
                continue
            },
            None => continue,
        };

        let fmt_log = match message.level {
            ArchivedLevel::Error => {
                format!(
                    "[\u{001b}[31;1mERROR\u{001b}[0m][{}:{}] > {}",
                    &*message.path, message.line, &*message.message
                )
            }
            ArchivedLevel::Warn => {
                format!(
                    "[\u{001b}[33;1mWARN\u{001b}[0m][{}:{}] > {}",
                    &*message.path, message.line, &*message.message
                )
            }

            ArchivedLevel::Info => {
                format!(
                    "[\u{001b}[34;1mINFO\u{001b}[0m][{}:{}] > {}",
                    &*message.path, message.line, &*message.message
                )
            }
            ArchivedLevel::Debug => {
                format!(
                    "\u{001b}[4;1m[DEBUG][{}:{}]\u{001b}[0m > {}",
                    &*message.path, message.line, &*message.message
                )
            }
            ArchivedLevel::Trace => {
                format!(
                    "\u{001b}[38;2;128;128;128;2m[TRACE][{}:{}] > {}\u{001b}[0m",
                    &*message.path, message.line, &*message.message
                )
            }
        };

        stdout.write_all(fmt_log.as_bytes()).await?;
        stdout.write(b"\n").await?;
        drop(bytes.split_to(n));
    }
    Ok(())
}
