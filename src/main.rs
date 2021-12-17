use std::rc::Rc;
use std::time::Duration;

mod toy;

async fn async_main(rt: Rc<toy::Runtime>) {
    println!("I am sleepy!");
    toy::sleep(&rt, Duration::from_millis(1000)).await;
    println!("I am awake, I am awake!");

    println!("I want sleep again...");
    rt.nested_loop(toy::sleep(&rt, Duration::from_millis(1000)));
    println!("Now I am awake!");
}

fn main() {
    toy::run(|rt| async_main(rt));
}
