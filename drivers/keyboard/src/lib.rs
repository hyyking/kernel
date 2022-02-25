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
    pub fn new() -> Self {
        Self {
            waker: AtomicWaker::new(),
        }
    }

    pub unsafe fn add_value(&self, value: u8) {
        QUEUE.push(value).expect("queue is full");
        self.waker.wake();
    }

    pub unsafe fn read_value() -> u8 {
        QUEUE.pop().expect("queue is full")
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
