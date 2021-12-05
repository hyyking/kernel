#![no_std]

extern crate alloc;

#[macro_use]
extern crate log;

use alloc::{boxed::Box, collections::VecDeque};

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

struct SchedulerWaker;

impl SchedulerWaker {
    const VTABLE: &'static RawWakerVTable =
        &RawWakerVTable::new(Self::clone, Self::wake, Self::wake_by_ref, Self::drop);

    pub fn waker() -> Waker {
        unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), Self::VTABLE)) }
    }

    unsafe fn clone(ptr: *const ()) -> RawWaker {
        todo!()
    }

    unsafe fn wake(ptr: *const ()) {
        todo!()
    }

    unsafe fn wake_by_ref(ptr: *const ()) {
        todo!()
    }

    unsafe fn drop(ptr: *const ()) {}
}

pub struct Task {
    task: Pin<Box<dyn Future<Output = ()>>>,
}

impl Future for Task {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.task.as_mut().poll(cx)
    }
}

impl Task {
    pub fn new<F>(task: F) -> Self
    where
        F: Future<Output = ()> + 'static,
    {
        Self {
            task: Box::pin(task),
        }
    }
}

#[derive(Default)]
pub struct Scheduler {
    tasks: VecDeque<Task>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: VecDeque::new(),
        }
    }

    pub fn spawn<F>(&mut self, task: F)
    where
        F: Future<Output = ()> + 'static,
    {
        self.tasks.push_back(Task::new(task))
    }

    pub fn run(&mut self) {
        while let Some(mut task) = self.tasks.pop_front() {
            let waker = SchedulerWaker::waker();
            let mut context = Context::from_waker(&waker);
            match Pin::new(&mut task).poll(&mut context) {
                Poll::Ready(a) => debug!("task ended {:?}", a),
                Poll::Pending => self.tasks.push_back(task),
            }
        }
    }
}
