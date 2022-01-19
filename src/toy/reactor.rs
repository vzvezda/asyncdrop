use std::cell::RefCell;
use std::task::Waker;
use std::time::{Duration, Instant};

// ID of the event in the reactor. This is a toy reactor, the only event is timer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct EventId(u32);

#[derive(Clone, Debug)]
pub struct Wait {
    pub event_id: EventId,
    pub waker: Waker,
}

impl Wait {
    fn new(event_id: EventId, waker: Waker) -> Self {
        Self { event_id, waker }
    }
}

pub struct Reactor {
    inner: RefCell<ReactorInner>,
}

impl Reactor {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(ReactorInner::new()),
        }
    }

    /// Adds timer into reactor
    pub(super) fn add_timer(&self, waker: &Waker, duration: Duration) -> EventId {
        self.inner.borrow_mut().add_timer(waker, duration)
    }

    /// Cancel the timer by id. Panics if there is no timer with given id
    pub(super) fn cancel_timer(&self, event_id: EventId) {
        self.inner.borrow_mut().cancel_timer(event_id)
    }

    /// Waits (sleeps) for a first timer to occurs. Returns None if there is no timers to wait.
    pub(super) fn wait(&self) -> Option<Wait> {
        self.inner.borrow_mut().wait()
    }
}

#[derive(Clone)]
struct Timer {
    event_id: EventId,
    awake_on: Instant,
    waker: Waker,
}

impl Timer {
    fn new(event_id: EventId, waker: &Waker, duration: Duration) -> Self {
        Self {
            event_id,
            awake_on: Instant::now() + duration,
            waker: waker.clone(),
        }
    }
}

struct ReactorInner {
    timers: Vec<Timer>,
    last_event_id: u32,
}

impl ReactorInner {
    pub fn new() -> Self {
        Self {
            timers: Vec::new(),
            last_event_id: 0,
        }
    }

    /// Adds timer into reactors.
    pub fn add_timer(&mut self, waker: &Waker, duration: Duration) -> EventId {
        self.last_event_id += 1;
        self.timers
            .push(Timer::new(EventId(self.last_event_id), waker, duration));

        EventId(self.last_event_id)
    }

    /// Cancel the timer by id. Panics if event_id is unknown.
    pub fn cancel_timer(&mut self, event_id: EventId) {
        // todo: maybe we should also make sure that event is removed from runtime.frozen_events.
        let index = self
            .timers
            .iter()
            .position(|timer| timer.event_id == event_id)
            .expect("Canceled unknown timer");

        self.timers.remove(index);
    }

    pub fn wait(&mut self) -> Option<Wait> {
        // This reactor IO is only timer.
        // Looking for a first timer to awake on
        let index = self
            .timers
            .iter()
            .enumerate() // [(position, Timer)]
            .min_by(|&l, &r| l.1.awake_on.cmp(&r.1.awake_on)) // Option<(position, Timer)>
            .map(|pair| pair.0); // Option(position)

        if let Some(index) = index {
            let Timer {
                event_id,
                awake_on,
                waker,
            } = self.timers.remove(index);

            let now = Instant::now();
            if now < awake_on {
                std::thread::sleep(awake_on - now);
            }

            Some(Wait::new(event_id, waker))
        } else {
            None // No events to wait
        }
    }
}
