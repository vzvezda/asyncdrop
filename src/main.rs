use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

mod reactor;
mod runtime;
mod sleep;

use crate::sleep::sleep;

async fn async_main(rt: Rc<runtime::Runtime>) {
    println!("I am sleepy!");
    sleep(rt, Duration::from_millis(1000)).await;
    println!("I am awake, I am awake!");
}

fn starter(rt: Rc<runtime::Runtime>) -> impl Future<Output = ()> {
    async_main(rt)
}

fn main() {
    runtime::run(starter);
}
