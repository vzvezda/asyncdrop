use super::task::{GuardedTask, Task};
use super::Runtime;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use pin_project::pin_project;

// Make a future that completes as soon as both futures are completed. Unlike other `join!`, this
// one also creates tasks, so .
// This is currently the only way to create tasks in this toy runtime.
pub fn make_rt_join2<'f1, 'f2, FutT1, FutT2>(
    rt: &Rc<Runtime>,
    f1: FutT1,
    f2: FutT2,
) -> RtJoin2<FutT1, FutT2>
where
    FutT1: Future<Output = ()> + 'f1,
    FutT2: Future<Output = ()> + 'f2,
{
    RtJoin2::<FutT1, FutT2>::new(rt, f1, f2)
}

#[pin_project]
pub struct RtJoin2<FutT1, FutT2>
where
    FutT1: Future<Output = ()>,
    FutT2: Future<Output = ()>,
{
    task1: GuardedTask,
    task2: GuardedTask,

    // Makes RtJoin2 to look like it owns FutT1 and FutT2 for borrow checker. If future has
    // references borrow checker would complain whenever user attempt RtJoin2 to outlive these.
    _lifetime1: PhantomData<FutT1>,
    _lifetime2: PhantomData<FutT2>,
}

impl<FutT1, FutT2> RtJoin2<FutT1, FutT2>
where
    FutT1: Future<Output = ()>,
    FutT2: Future<Output = ()>,
{
    fn new(rt: &Rc<Runtime>, f1: FutT1, f2: FutT2) -> Self {
        Self {
            task1: unsafe { Task::allocate(rt, f1) },
            task2: unsafe { Task::allocate(rt, f2) },
            _lifetime1: PhantomData,
            _lifetime2: PhantomData,
        }
    }

    fn is_completed(&self) -> bool {
        self.task1.task.is_completed() && self.task2.task.is_completed()
    }
}

impl<FutT1, FutT2> Future for RtJoin2<FutT1, FutT2>
where
    FutT1: Future<Output = ()>,
    FutT2: Future<Output = ()>,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_ref().project_ref();

        if self.is_completed() {
            return Poll::Ready(());
        }

        this.task1.task.poll_child(ctx);
        if self.is_completed() {
            return Poll::Ready(());
        }

        this.task2.task.poll_child(ctx);
        if self.is_completed() {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}
