use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake};

use super::Runtime;

pub(super) enum TaskPoll {
    Pending,
    Ready,
    Frozen,
    Gone,
}

// Helps to destroy task's future in a right time when all references are still valid.
pub(super) struct GuardedTask {
    pub task: Arc<Task>,
}

impl Drop for GuardedTask {
    fn drop(&mut self) {
        self.task.destroy();
    }
}

pub(super) struct Task {
    future: RefCell<Option<Pin<Box<dyn Future<Output = ()>>>>>,
    parent: RefCell<Option<Arc<Task>>>,
    awoken_task: Arc<RefCell<Option<Arc<Task>>>>,
    completed: Cell<bool>,
}

// Added these to fix compliation error while working with std::task::Wake. This
// toy runtime does not use threads and Task is not public, so unsafe should be ok.
unsafe impl Sync for Task {}
unsafe impl Send for Task {}

impl Task {
    // Creates task from the future.
    //
    // unsafe: task has future lifetime erased ('f -> 'static) with unsafe transmute. It is up
    // to the caller to ensure that allocated task object does not outlive the 'f, e.g.
    // objects referenced in the futures. This unsafeness is not exposed to app, it should be
    // internal thing.
    pub(super) unsafe fn allocate<'f, FutT>(rt: &Runtime, f: FutT) -> GuardedTask
    where
        FutT: Future<Output = ()> + 'f,
    {
        // Make the box and erase lifetime
        let boxed_f: Pin<Box<dyn Future<Output = ()> + 'f>> = Box::pin(f);
        let boxed_f: Pin<Box<dyn Future<Output = ()> + 'static>> = std::mem::transmute(boxed_f);

        GuardedTask {
            task: Arc::new(Self {
                future: RefCell::new(Some(boxed_f)),
                awoken_task: rt.awoken_task.clone(),
                parent: RefCell::new(None),
                completed: Cell::new(false),
            }),
        }
    }

    // destroy is used to drop the future
    pub fn destroy(&self) {
        // panics if self.future is already borrowed: it should never happens unless there is
        // a bug in crate.
        *self.future.borrow_mut() = None; // drop the future
        *self.parent.borrow_mut() = None; // dec counter for parent
    }

    // Assigns parent to task
    fn assign_parent(&self, parent_context: Option<&mut Context<'_>>) {
        if parent_context.is_some() {
            let mut parent = self.parent.borrow_mut();
            parent.get_or_insert(self.current_task(parent_context.unwrap()));
        }
    }

    // Extracts task from Context
    fn current_task(&self, ctx: &mut Context<'_>) -> Arc<Task> {
        // By invoking wake() we have Arc<Task> written to self.awoken_task.
        ctx.waker().clone().wake();
        self.awoken_task.borrow_mut().take().unwrap().clone()
    }

    // If current task is frozen
    pub fn is_frozen(&self) -> bool {
        match self.future.try_borrow_mut() {
            Err(_) => true,
            Ok(_) => false,
        }
    }

    // Polls a root task, e.g.  the task without parent. Root task is created by run() or
    // nested_loop(), so there can be more than one root task.
    pub fn poll(self: &Arc<Self>) -> TaskPoll {
        self.poll_impl(None)
    }

    // Polls the child future and currently only used by join. Context is used to assign
    // parent task.
    pub fn poll_child(self: &Arc<Self>, parent_context: &mut Context<'_>) -> TaskPoll {
        self.poll_impl(Some(parent_context))
    }

    // Shared impl of poll for poll() and poll_child().
    fn poll_impl(self: &Arc<Self>, parent_context: Option<&mut Context<'_>>) -> TaskPoll {
        match self.future.try_borrow_mut() {
            Err(_) => TaskPoll::Frozen,
            Ok(mut future) => {
                if future.is_none() {
                    // Future is out out scope and had been deleteded. This must be some
                    // some call from frozen_event array.
                    return TaskPoll::Gone;
                }

                if self.completed.get() {
                    // Future has been completed.
                    return TaskPoll::Ready;
                }

                // we have borrowed the future, so it is now "frozen" by this scope. Once we
                // reenter this function (e.g. by nested_loop()), it would return Frozen.

                // If this is a subtask (created by join) on first poll we may need to assign
                // the parent and thus keep a "task forest" data struct. It is forest because
                // multiple roots can be created by nested_loop().
                self.assign_parent(parent_context);

                let waker = self.clone().into();
                let mut ctx = Context::from_waker(&waker);
                match future.as_mut().unwrap().as_mut().poll(&mut ctx) {
                    Poll::Ready(()) => {
                        self.completed.set(true);
                        TaskPoll::Ready
                    }
                    Poll::Pending => TaskPoll::Pending,
                }
            }
        }
    }

    // Find a closest unfronzen parent
    pub fn first_unfrozen_parent(self: &Arc<Self>) -> Arc<Self> {
        if self.is_frozen() || self.parent.borrow().is_none() {
            // 1. When task has no parent we can only return self
            // 2. When the task is frozen parents must have been frozen as well, so return
            //    self.
            self.clone()
        } else {
            let parent = self.parent.borrow();
            let parent = parent.as_ref().unwrap();
            if parent.is_frozen() {
                self.clone()
            } else {
                parent.first_unfrozen_parent()
            }
        }
    }

    // If future had poll with Poll::Ready
    pub fn is_completed(&self) -> bool {
        match self.future.try_borrow() {
            Err(_) => false,
            Ok(future) => self.completed.get(),
        }
    }
}

// This is how this runtime implement Waker
impl Wake for Task {
    fn wake(self: Arc<Self>) {
        *(self.awoken_task.borrow_mut()) = Some(self.clone());
    }
}
