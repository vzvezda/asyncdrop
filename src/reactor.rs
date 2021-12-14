use std::cell::RefCell;
use std::task::Waker;
use std::time::{Duration, Instant};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TimerId(u32);

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
    pub fn add_timer(&self, waker: &Waker, duration: Duration) -> TimerId {
        self.inner.borrow_mut().add_timer(waker, duration)
    }

    /// Cancel the timer by id. Panics if there is no timer with given id
    pub fn cancel_timer(&self, timer_id: TimerId) {
        self.inner.borrow_mut().cancel_timer(timer_id)
    }

    /// Waits (sleeps) for a first timer to occurs. Returns None if there is no timers to wait.
    pub fn wait(&self) -> Option<TimerId> {
        self.inner.borrow_mut().wait()
    }
}

#[derive(Clone)]
struct Timer {
    timer_id: TimerId,
    awake_on: Instant,
    waker: Waker,
}

impl Timer {
    fn new(timer_id: TimerId, waker: &Waker, duration: Duration) -> Self {
        Self {
            timer_id,
            awake_on: Instant::now() + duration,
            waker: waker.clone(),
        }
    }
}

struct ReactorInner {
    timers: Vec<Timer>,
    last_timer_id: u32,
}

impl ReactorInner {
    pub fn new() -> Self {
        Self {
            timers: Vec::new(),
            last_timer_id: 0,
        }
    }

    /// Adds timer into reactors.
    pub fn add_timer(&mut self, waker: &Waker, duration: Duration) -> TimerId {
        self.last_timer_id += 1;
        self.timers
            .push(Timer::new(TimerId(self.last_timer_id), waker, duration));

        TimerId(self.last_timer_id)
    }

    /// Cancel the timer by id. Panics if timer_id is unknown.
    pub fn cancel_timer(&mut self, timer_id: TimerId) {
        let index = self
            .timers
            .iter()
            .position(|timer| timer.timer_id == timer_id)
            .expect("Canceled unknown timer");

        self.timers.remove(index);
    }

    pub fn wait(&mut self) -> Option<TimerId> {
        // looking for a first timer to awake on
        let index = self
            .timers
            .iter()
            .enumerate() // [(position, Timer)]
            .min_by(|&l, &r| l.1.awake_on.cmp(&r.1.awake_on)) // Option<(position, Timer)>
            .map(|pair| pair.0); // Option(position)

        if let Some(index) = index {
            let Timer {
                timer_id,
                awake_on,
                waker,
            } = self.timers.remove(index);

            let now = Instant::now();
            if now < awake_on {
                std::thread::sleep(awake_on - now);
            }

            Some(timer_id)
        } else {
            None // No events to wait
        }
    }
}
