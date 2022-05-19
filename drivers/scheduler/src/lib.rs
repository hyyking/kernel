#![no_std]
#![feature(allocator_api)]

extern crate alloc;

#[macro_use]
extern crate log;

use alloc::{alloc::Allocator, boxed::Box, collections::VecDeque};

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use kalloc::shared::SharedAllocator;

struct SchedulerWaker;

impl SchedulerWaker {
    const VTABLE: &'static RawWakerVTable =
        &RawWakerVTable::new(Self::clone, Self::wake, Self::wake_by_ref, Self::drop);

    pub fn waker() -> Waker {
        unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), Self::VTABLE)) }
    }

    unsafe fn clone(_ptr: *const ()) -> RawWaker {
        RawWaker::new(core::ptr::null(), Self::VTABLE)
    }

    unsafe fn wake(_ptr: *const ()) {
        todo!()
    }

    unsafe fn wake_by_ref(_ptr: *const ()) {
        todo!()
    }

    unsafe fn drop(_ptr: *const ()) {}
}

type TaskFuture = dyn Future<Output = ()>;

pub struct Task<A>
where
    A: Allocator,
{
    task: Pin<Box<TaskFuture, SharedAllocator<A>>>,
}

impl<A> Future for Task<A>
where
    A: Allocator + 'static,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.task.as_mut().poll(cx)
    }
}

impl<A> Task<A>
where
    A: Allocator + 'static,
{
    pub fn new<F>(task: F, alloc: SharedAllocator<A>) -> Self
    where
        F: Future<Output = ()> + 'static,
    {
        Self {
            task: Box::pin_in(task, alloc),
        }
    }
}

pub struct Scheduler<A: Allocator> {
    tasks: VecDeque<Task<A>>,
    alloc: SharedAllocator<A>,
}

impl<A> Scheduler<A>
where
    A: Allocator + 'static,
{
    pub fn new(alloc: A) -> Self {
        Self {
            tasks: VecDeque::new(),
            alloc: SharedAllocator::new(alloc),
        }
    }

    pub fn spawn<F>(&mut self, task: F)
    where
        F: Future<Output = ()> + 'static,
    {
        self.tasks.push_back(Task::new(task, self.alloc.clone()));
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
