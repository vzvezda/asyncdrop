use super::task::{GuardedTask, Task};
use super::Runtime;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use pin_project::pin_project;

// Make a future that completes as soon as both futures are completed. This join
// does not create any tasks.
pub fn make_join2<FutT1, FutT2>(f1: FutT1, f2: FutT2) -> Join2<FutT1, FutT2>
where
    FutT1: Future<Output = ()>,
    FutT2: Future<Output = ()>,
{
    Join2::<FutT1, FutT2>::new(f1, f2)
}

#[pin_project]
pub struct Join2<FutT1, FutT2>
where
    FutT1: Future<Output = ()>,
    FutT2: Future<Output = ()>,
{
    #[pin]
    fut1: FutT1,
    #[pin]
    fut2: FutT2,

    fut1_done: bool,
    fut2_done: bool,
}

impl<FutT1, FutT2> Join2<FutT1, FutT2>
where
    FutT1: Future<Output = ()>,
    FutT2: Future<Output = ()>,
{
    fn new(f1: FutT1, f2: FutT2) -> Self {
        Self {
            fut1: f1,
            fut2: f2,
            fut1_done: false,
            fut2_done: false,
        }
    }

    fn is_completed(&self) -> bool {
        self.fut1_done && self.fut2_done
    }
}

impl<FutT1, FutT2> Future for Join2<FutT1, FutT2>
where
    FutT1: Future<Output = ()>,
    FutT2: Future<Output = ()>,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().project();

        match this.fut1.poll(ctx) {
            Poll::Ready(_) => {
                *this.fut1_done = true;
                if *this.fut2_done {
                    return Poll::Ready(());
                }
            }
            _ => (),
        }

        match this.fut2.poll(ctx) {
            Poll::Ready(_) => {
                *this.fut2_done = true;
                if *this.fut1_done {
                    return Poll::Ready(());
                }
            }
            _ => (),
        }

        Poll::Pending
    }
}
