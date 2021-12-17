use std::cell::Cell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake};

use super::reactor::TimerId;
use crate::toy::Reactor;

pub struct Runtime {
    reactor: Reactor,
    awoken: Rc<Cell<Option<TimerId>>>,
}

impl Runtime {
    fn new() -> Self {
        Runtime {
            reactor: Reactor::new(),
            awoken: Rc::new(Cell::new(None)),
        }
    }

    pub fn nested_loop<FutT>(&self, fut: FutT)
    where
        FutT: Future<Output = ()>,
    {
        let mut fut = Box::pin(fut);

        let waker = Arc::new(ZeroWaker).into();
        let mut ctx = Context::from_waker(&waker);

        loop {
            match fut.as_mut().poll(&mut ctx) {
                Poll::Pending => self.awoken.set(Some(self.reactor().wait().unwrap())),
                Poll::Ready(_) => return,
            }
        }

        // can we have here a Pin<&mut RootFuture>?
        todo!()
    }

    pub fn reactor(&self) -> &Reactor {
        &self.reactor
    }

    pub fn is_awoken(&self, timer_id: TimerId) -> bool {
        self.awoken.get().map_or(false, |id| id == timer_id)
    }

    fn block_on<FutT>(&self, fut: FutT)
    where
        FutT: Future<Output = ()>,
    {
        let mut fut = Box::pin(fut);

        let waker = Arc::new(ZeroWaker).into();
        let mut ctx = Context::from_waker(&waker);

        loop {
            match fut.as_mut().poll(&mut ctx) {
                Poll::Pending => self.awoken.set(Some(self.reactor().wait().unwrap())),
                Poll::Ready(_) => return,
            }
        }
    }
}

pub fn run<StarterFn, FutT>(starter: StarterFn)
where
    StarterFn: FnOnce(Rc<Runtime>) -> FutT,
    FutT: Future<Output = ()>,
{
    let rt = Rc::new(Runtime::new());
    let future = starter(rt.clone());
    rt.block_on(future);
}

struct ZeroWaker;

impl Wake for ZeroWaker {
    fn wake(self: Arc<Self>) {}
}
