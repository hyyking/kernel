#![no_std]

use core::{
    pin::Pin,
    task::{Context, Poll},
};

use kcore::{futures::task::AtomicWaker, queue::ArrayQueue};

kcore::klazy! {
    ref static QUEUE: ArrayQueue<u8> = { ArrayQueue::new(4) };
}

pub struct Keyboard {
    waker: AtomicWaker,
}

impl Keyboard {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            waker: AtomicWaker::new(),
        }
    }

    #[inline]
    pub fn add_value(&self, value: u8) {
        QUEUE.push(value).expect("queue is full");
        self.waker.wake();
    }

    #[inline]
    #[must_use]
    pub fn read_value() -> Option<u8> {
        QUEUE.pop()
    }
}

impl kcore::futures::stream::Stream for Keyboard {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match QUEUE.pop() {
            key @ Some(_) => Poll::Ready(key),
            None => {
                self.waker.register(cx.waker());
                Poll::Pending
            }
        }
    }
}
