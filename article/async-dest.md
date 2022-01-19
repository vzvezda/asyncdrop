# Async destruction on stable rust

Async destruction is idea that exists as proposals to rust language to solve some of the corner cases of future cancellation. A typical case is that a future that owns the buffer and has scheduled the I/O operation with kernel and now someone drops the future (for example by using `select!`), so in future's `fn drop(&mut self)` we have this issue that we should notify the kernel about cancelling the pending I/O and then keep the buffer alive until kernel confirms the cancellation. This means that `fn drop(&mut self)` has to wait kernel async cancellation to complete and it usually means that no other work is possible in this thread. 

Known workaround to this is to make `Future` sync drop safe: if a buffer must be valid during async cancellation it either owned by async runtime or transferred to runtime which can safely await kernel to confirm cancellation after Future has been dropped. Another idea such as blocking the thread on `fn drop(&mut self)` waiting for async IO cancellation to complete looks too impractical to use. Imagine that that your million clients WebSocket chat blocked because of waiting a cleanup of some connection.

This article is to explore if we can have a `drop()` that able to keep Future-owned buffer alive while not stalling the async code execution in a single-thread executor.

## Setup

Let's have a toy single-thread runtime for this research. This drafts the idea I plan to explore in this article:

```rust
// Our toy runtime is here
mod toy {
    // Use it to schedule i/o to kernel
    pub struct Reactor;
    // Runtime is executor + reactor
    pub struct Runtime {
        // ...
        reactor: Reactor,
    }

    impl Runtime {
        /// This is our function of interest to support async destruction
        pub fn nested_loop<FutT: Future>(&self, fut: FutT) -> FutT::Output {
            todo!("Do not return until fut is completed")
        }

        // API to schedule I/O
        pub fn io(&self) -> &Reactor {
            &self.reactor
        }
    }

    pub async fn with_timeout<FutT: Future>(fut: FutT, timeout: Duration) {
        todo!("Run the future and drop it if timeout happens before completion")
        // like select!-ing { fut, sleep(timeout) }
    }

    // This is analog of "block_on" but it requires a sync function to receive 
    // Runtime instance.
    pub fn run<StarterFn, FutT>(starter: StarterFn)
    where
        StarterFn: FnOnce(Rc<Runtime>) -> FutT,
        FutT: Future,
    {
        todo!("Invoke a function that returns future and run it to completion")
    }
}

// Future that owns a buffer submitted to kernel
struct KernelRead {
    rt: Rc<toy::Runtime>,
    buffer: [u8; 1000],
    pending: bool,
    _marker: PhantomPinned,
}

impl Future for KernelRead {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        todo!("1. schedule rt.io().read_async(&buffer)");
        todo!("2. verify if future awoken because async read is ready")
    }
}

impl KernelRead {
    fn new(rt: &Rc<toy::Runtime>) -> Self {
        Self {
            rt: rt.clone(),
            buffer: [0; 1000],
            pending: false,
            _marker: PhantomPinned,
        }
    }

    async fn close(&mut self) {
        if (self.pending) {
            todo!("Schedule rt.io().cancel_async_read(&buffer) and wait");
        }
    }
}

impl Drop for KernelRead {
    fn drop(&mut self) {
        // We cannot drop until our async cancellation not finished but we
        // would like to poll
        self.rt.clone().nested_loop(self.close());
    }
}

// In async main we receive runtime
async fn async_main(rt: Rc<toy::Runtime>) {
    toy::with_timeout(KernelRead::new(&rt), Duration::from_secs(5)).await;
}

fn main() {
    toy::run(|rt| async_main(rt));
}
```

First let's address the concern that Rust has `mem::forget()` that makes it possible to have object destroyed without running it destructor. So one can deallocate `KernelRead` while OS is writing into it. But we have the `Pin` for the rescue: its "Drop guarantee" says that "its memory will not get invalidated or repurposed from the moment it gets pinned until when drop is called". As the `KernelRead` is !Unpin we can be sure that once I/O was scheduled (thus future was pinned), it either the `KernelRead` memory is valid or its `drop()` was called.

So when execution is in the destructor for `KernelRead` future it has to wait until kernel confirms it no longer writes into the buffer. `nested_loop()` function runs until kernel confirms that this is ok to have memory region de-allocated. So looks like `drop()` is blocked, but can we have execution of other async tasks continued?

## N = 0

Lets look at this example.

```rust
async fn cleanup(rt: &Rc<toy::Runtime>) { /* ... */ }
async fn async_job1(rt: &Rc<toy::Runtime>) {  /* ... */ }
async fn async_job2(rt: &Rc<toy::Runtime>) {  /* ... */ }

async fn async_main(rt: Rc<toy::Runtime>) {
    async_job1(&rt).await;
    rt.clone().nested_loop(cleanup(&rt));
    async_job2(&rt).await;    
}

toy::run(|rt| async_main(rt));

```

While the `nested_loop()`  supposed to be invoked from Future's `fn drop(&mut self)`  it is not a requirement and in the code examples of this article it is invoked whenever it convenient for clarity. 

So what we can see:

* `async_main()` is a root future polled by `toy::run` until completion
* `cleanup()` is another future that is polled by `nested_loop() ` until completion
* There is some async code before  `nested_loop() ` and after 

How we would like the `nested_loop()` to behave in this case? Let's assume that our toy::Runtime enforces Structured Concurrency, which means that whatever happened `async_job1(&rt).await;` line is completely finished and there is no event in reactor that can be landed anywhere. So how our `nested_loop()` supposed to work? It can work just like `toy::run()` just poll the `cleanup` future and wait until reactor has an event ready.

## N = N + 1

The code snippet in section above is async but it is sequential, which means that is no parallelism. There is nothing `nested_loop()` can do while it waits for the `cleanup()` future to complete. Usually async executors provide parallelism using two methods:

* `spawn()` function that creates a new async task (see `tokio::spawn` or `async_std::task::spawn`)
* `join!/select!()` macros to run futures concurrently without making new task

I am going to investigate further the `join!/select!()` approach and as for `spawn()` I think that it should be even easier. 

Consider this code example:

```rust
async fn cleanup(rt: &Rc<toy::Runtime>) { /* ... */ }

async fn foo(rt: &Rc<toy::Runtime>) { /* ... */ }

async fn bar(rt: &Rc<toy::Runtime>) {
    // run the nested loop here
    rt.clone().nested_loop(cleanup(rt));
}

async fn async_main(rt: Rc<toy::Runtime>) {
    toy::make_rt_join2(&rt, foo(&rt), bar(&rt)).await;
}

toy::run(|rt| async_main(rt));
```

For further progress we need to have our own version of `join/select`, because these that are from `futures` or `tokio` crate does not work for us. The join macro for `toy` library would be `toy::make_rt_join2` function that takes runtime and two futures and invoke `poll()` on these until both are completed. 

Unlike `join!` our `make_rt_join2()` creates new tasks for its branches. What does I mean saying that "`make_rt_join2()` creates new tasks"? Well, I mean that `joy::Runtime` now aware of this branches and has some flexibility when to run these. New task does not mean it has own thread, our `toy` executor is single threaded. 

So we can draw the task structure seen by `toy::Runtime` once code execution arrives to `nested_loop()`:

![task_flow](img/task_flow.svg)

So callstack is shown by arrows and be like:

```rust
rt.nested_loop(clean_up)
<bar as Future>::poll(...)
<join as Future>::poll(...)
<async_main as Future>::poll(...)
toy::run(...)
main()
```

We would like the `nested_loop()` not exit until `cleanup()` is completed. But now while we are waiting for the `cleanup()` to finish we can also poll the `foo()` feature! `nested_loop()` cannot poll `async_main` and  `bar` because these are uniquely borrowed by theirs poll method in callstack, but not nobody uses `foo as Future`, so we can temporarily borrow it to our `nested_loop()`:

![task_flow](img/borrowed-and-frozen.svg)

The snowflake icon on future means that it is "frozen":

* Runtime cannon invoke this future's  `poll()` method in `nested_loop():1` because somewhere in the callstack it was already uniquely borrowed by `poll()` method
* The frozen future can still have some I/O scheduled (like if it uses `futures::join!/select!`). When reactor awakes with event for frozen future, executor is unable to poll it, it should store the awakening event somewhere and it has to be consumed later when future will be "unfrozen". 

Lets look on these two possible outcomes:

* `cleanup()` completed first: it means that `nested_loop()` unborrows the "`foo*()`" and exits. Execution returns up to `bar::poll()` which returns `Poll::Ready(())` and `join::poll()` returns `Poll::Pending` to `toy::run()`. 
* `foo*()` completed first: runtime saves result somewhere and the `nested_loop():1` keeps polling only the `cleanup()`. Once `cleanup()` is done and `bar::poll()` is `Poll::Ready(())`, execution returned `join::poll()` that can now return `Poll::Ready(())` to `toy::run()` as both its futures are completed. 

So far it looks easy. However these are not the only possible outcomes, what happens when while we have `nested_loop()` someone invokes the `nested_loop()` again?

## Recursive nesting

So, what we going to have once in `nested_loop()` we have entered into another `nested_loop()`. For example feature `foo()` in our example also invokes `cleanup_foo()`:

![task_flow](img/nested_loop()_2.svg)

In this case nested_loop will have two features to poll, `cleanup()` and `cleanup_foo()`:

![task_flow](img/nested_loop()_2_run.svg)

Let's discuss these possible outcomes:

* `cleanup_foo()` completes first and it means that nested_loop():2 can exit. The `nested_loop()`:1 has still poll the `cleanup()` until it finish.

* `cleanup()` completes first: well, this is the interesting consequence of this approach that `nested_loop():2` cannot exit until `cleanup_foo()` is completed. Which means that even if future `bar()` is ready to return `Poll::Ready()`, it's result cannot be delivered because some other feature from task tree has launched `nested_loop()`.

  It looks very concerning.  Just imagine that you have several `nested_loop()` in the callstack and some of these have their future completed but cannot return because of `nested_loop()` somewhere in the top of the callstack. Does it makes this approach impractical? Perhaps, but I am not convinced yet, I think it requires further verification. 

As for recursive execution, the `cleanup()` futures just like any other futures can have `join!/select!` and `nested_loop()` internally. It makes the runtime execution structure more complex but It does not make the program incorrect. When cleanup invokes `nested_loop()` it become frozen and cannot be polled until internal `nested_loop()` exits.

## Selecting

So far we only discussed running branches with `join`. What if we have some kind of `select!`-like construct when a first completed branch can cause futures in other branches to drop? Let's look on task structure with select instead of join:

![task_flow](img/selecting.svg)

It looks much like the `join!` but what happens that while we are in `nested_loop()` we have discovered that `foo()` is completed? Should we still deliver reactor events and poll `baz()`? I think it depends on how we would implement `select` for toy runtime. If it is known that completing one of the branches drops the others it does not make any sense to poll `baz()` anymore. 

## Discussion

There are some number of thing to dislike and to be concerned with proposed approach, here is my top 4:

* Building the mental model for how your program works become even more difficult as you should be aware that recursive `nested_loop()` happens
* We can have some performance cost attached to new tasks-aware `select!`/`join`! and for keeping reactor's events for frozen futures.
* It is possible to have stack overflow if the program happens to generate `nested_loop()` faster than it consumes these.
* We can observer this effect that some tasks takes more time to complete because of nested_loop() launched by another task.

However I think it still worth some further research just to verify if any of these concerns would actually create some pain.

Why do we need the async destruction in a first place? I think that this is to connect the sync nature of future destruction to async nature of OS async cancellation API. If async cancellation is rare and short operation it can be that most of the time app did not notice it has `nested_loop()`s. And in optimistic scenario if we have some kind of AsyncDrop landed to rust-lang this `nested_loop()` hack can be dropped without much efforts.

## Implementation

I have implemented a proof of concept toy runtime and demo app with `nested_loop()` that can be found in this repository: https://github.com/vzvezda/asyncdrop/. 

## Links

[1] https://boats.gitlab.io/blog/post/poll-drop/ "Asynchronous Destructors", withoutboats, October 16, 2019

[2] https://internals.rust-lang.org/t/asynchronous-destructors/11127 discussion of the article above

[3] https://boats.gitlab.io/blog/post/io-uring/, specifically the section "Blocking on drop does not work", May 6, 2020

[4] https://internals.rust-lang.org/t/pre-rfc-leave-auto-trait-for-reliable-destruction/13825 discussion of alternative proposal that also related to async destruction, January 2021
