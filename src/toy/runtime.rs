// TODO:
//   * fix warnings
//   * fix code (add finished comments)
//   * to git
//   * publish
//

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;

use super::reactor::EventId;
use super::reactor::Wait;
use super::task::Task;
use super::task::TaskPoll;
use crate::toy::Reactor;

pub struct Runtime {
    reactor: Reactor,
    awoken_event: Cell<Option<EventId>>,
    frozen_events: RefCell<Vec<Wait>>,

    // Need this visible for Waker/Task
    pub(super) awoken_task: Arc<RefCell<Option<Arc<Task>>>>,
}

impl Runtime {
    fn new() -> Self {
        Runtime {
            reactor: Reactor::new(),
            awoken_task: Arc::new(RefCell::new(None)),
            awoken_event: Cell::new(None),
            frozen_events: RefCell::new(Vec::new()),
        }
    }

    // Function that runs the nested poll loop making async destruction possible without
    // blocking all the tasks. So it starts the cleanup as a new task and poll all task
    // it can until cleanup is completed.
    pub fn nested_loop<FutT>(&self, cleanup: FutT)
    where
        FutT: Future<Output = ()>,
    {
        let cleanup_task = unsafe { Task::allocate(self, cleanup) };

        // Poll future once to give it chance to schedule its i/o in reactor
        match cleanup_task.task.poll() {
            TaskPoll::Ready => return,
            _ => (),
        }

        // Now wait for events from reactor to wake up unfrozen tasks
        loop {
            // If there are any events that was scheduled for frozen task that now unfrozen
            // and can be polled.
            self.poll_frozen_events();
            // cleanup task can be completed by some other nested loop
            if cleanup_task.task.is_completed() {
                return;
            }

            let wait = self.reactor().wait().expect("Reactor.wait() has failed");

            self.awoken_event.set(Some(wait.event_id));
            wait.waker.clone().wake(); // sets self.awoken_task

            let awoken_task = self.awoken_task.borrow_mut().take().unwrap();
            let awoken_task = awoken_task.first_unfrozen_parent();

            match awoken_task.poll() {
                TaskPoll::Frozen => self.frozen_events.borrow_mut().push(wait),
                _ => (),
            }

            // cleanup task can be completed by some other nested loop
            if cleanup_task.task.is_completed() {
                return;
            }
        }
    }

    fn poll_frozen_events(&self) {
        while let Some((event_id, awoken_task)) = self.first_unfrozen_task() {
            println!("poll task from frozen_events");
            let awoken_task = awoken_task.first_unfrozen_parent();
            self.awoken_event.set(Some(event_id));

            match awoken_task.poll() {
                TaskPoll::Frozen => panic!("bug in first_unfrozen_task()/first_unfrozen_parent()"),
                TaskPoll::Gone => println!("poll the destroyed task, no-op"),
                _ => (),
            }
        }
    }

    // Scans the self.frozen_event and returns the first event that supposed to be delivered to
    // currently unfrozen task.
    fn first_unfrozen_task(&self) -> Option<(EventId, Arc<Task>)> {
        // find the first unfrozen task in self.frozen_events
        let pos_and_task = self
            .frozen_events
            .borrow_mut()
            .iter()
            .map(|wait| {
                // converts waker to Arc<Task>
                wait.waker.clone().wake();
                self.awoken_task.borrow_mut().take().unwrap()
            })
            .enumerate()
            .find(|(pos, task)| !task.is_frozen());

        // Remove event from frozen_events and return as (EventId, Arc<Task>)
        pos_and_task.map(|(pos, task)| {
            let Wait { event_id, waker } = self.frozen_events.borrow_mut().remove(pos);
            (event_id, task)
        })
    }

    pub fn reactor(&self) -> &Reactor {
        &self.reactor
    }

    pub fn is_awoken(&self, event_id: EventId) -> bool {
        self.awoken_event.get().map_or(false, |id| id == event_id)
    }

    // The block_on version is private and therefore is not reetrable
    fn block_on<FutT>(&self, fut: FutT)
    where
        FutT: Future<Output = ()>,
    {
        println!("block_on");
        self.nested_loop(fut)
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
