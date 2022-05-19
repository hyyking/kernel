mod codec;

use std::{cell::RefCell, collections::HashMap, io, rc::Rc};

use protocols::log::{ArchivedLevel, ArchivedLogPacket, Level};

use tokio::{io::AsyncWriteExt, net::TcpListener};

use tokio_util::codec::Decoder;

use kcore::futures::stream::StreamExt;

#[derive(Debug)]
struct Span {
    id: u64,
    target: String,
    messages: Vec<Message>,
}

#[derive(Debug)]
pub struct Message {
    pub level: Level,
    pub line: usize,
    pub module: String,
    pub message: String,
}

impl Span {
    #[must_use]
    fn new(id: u64, target: String) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            id,
            target,
            messages: vec![],
        }))
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let mut addr = std::env::args().skip(1);
    let addr = addr
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "missing server address"))?;

    let listener = TcpListener::bind(addr).await?;

    let (stream, _) = listener.accept().await?;
    stream.set_nodelay(true)?;
    stream.set_linger(None)?;

    let mut stdout = tokio::io::stdout();

    let mut spans = HashMap::<u64, Rc<RefCell<Span>>>::new();
    let mut span_stack = Vec::<Rc<RefCell<Span>>>::new();

    let mut framed = codec::LogDecoder::new().framed(stream);

    while let Some(message) = framed.next().await.transpose()? {
        let message = match message.as_ref() {
            ArchivedLogPacket::Message(message) => message,
            ArchivedLogPacket::NewSpan(span) => {
                spans.insert(span.id, Span::new(span.id, (&*span.target).to_string()));
                continue;
            }
            ArchivedLogPacket::EnterSpan(span) => {
                if let Some(span) = spans.get(span) {
                    span_stack.push(Rc::clone(span));
                    stdout
                        .write_all(
                            format!("OPEN: {} - {}\n", span.borrow().id, &*span.borrow().target)
                                .as_bytes(),
                        )
                        .await?;
                }
                continue;
            }
            ArchivedLogPacket::ExitSpan(span) => {
                if let Some(span) = spans.get(span) {
                    assert_eq!(
                        span_stack.pop().map(|s| s.borrow().id),
                        Some(span.borrow().id)
                    );
                    stdout
                        .write_all(
                            format!("CLOSE: {} - {}\n", span.borrow().id, &*span.borrow().target)
                                .as_bytes(),
                        )
                        .await?;
                }
                continue;
            }
        };

        const fn archive_to_level(archive: ArchivedLevel) -> Level {
            match archive {
                ArchivedLevel::Error => Level::Error,
                ArchivedLevel::Warn => Level::Warn,
                ArchivedLevel::Info => Level::Info,
                ArchivedLevel::Debug => Level::Debug,
                ArchivedLevel::Trace => Level::Trace,
            }
        }

        let level = archive_to_level(message.level);

        if let Some(last) = span_stack.last_mut() {
            last.borrow_mut().messages.push(Message {
                level,
                line: message.line as usize,
                module: String::from(&*message.path),
                message: String::from(&*message.message),
            })
        }

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

        for _ in 0..span_stack.len() {
            stdout.write_all(b" ").await?;
        }
        if !span_stack.is_empty() {
            stdout.write_all("↳".as_bytes()).await?;
        }
        stdout.write_all(fmt_log.as_bytes()).await?;
        let _ = stdout.write(b"\n").await?;
    }

    Ok(())
}
