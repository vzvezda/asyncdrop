use super::reactor::EventId;
use crate::toy::Runtime;

use pin_project::{pin_project, pinned_drop};

use std::future::Future;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

pub async fn sleep(rt: &Rc<Runtime>, duration: Duration) {
    Sleep::new(rt, duration).await
}

#[derive(Copy, Clone)]
enum PollState {
    Idle(Duration),
    Pending(EventId),
    Done,
}

#[pin_project(PinnedDrop)]
struct Sleep {
    rt: Rc<Runtime>,
    poll_state: PollState,
    _pinned: PhantomPinned,
}

impl Sleep {
    fn new(rt: &Rc<Runtime>, duration: Duration) -> Self {
        Self {
            rt: rt.clone(),
            poll_state: PollState::Idle(duration),
            _pinned: PhantomPinned,
        }
    }

    fn schedule(&mut self, duration: Duration, waker: &Waker) -> Poll<()> {
        self.poll_state = PollState::Pending(self.rt.reactor().add_timer(waker, duration));
        Poll::Pending
    }

    fn complete(&mut self, timer_id: EventId, waker: &Waker) -> Poll<()> {
        if self.rt.is_awoken(timer_id) {
            self.poll_state = PollState::Done;
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }

    fn cancel(&self, timer_id: EventId) {
        self.rt.reactor().cancel_timer(timer_id);
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_ref().project_ref();

        match *this.poll_state {
            PollState::Idle(duration) => self.schedule(duration, ctx.waker()),
            PollState::Pending(timer_id) => self.complete(timer_id, ctx.waker()),
            PollState::Done => panic!("polled the completed Sleep future"),
        }
    }
}

#[pinned_drop]
impl PinnedDrop for Sleep {
    fn drop(mut self: Pin<&mut Self>) {
        let this = self.as_ref().project_ref();

        match this.poll_state {
            PollState::Pending(timer_id) => self.cancel(*timer_id),
            _ => (),
        }
    }
}
