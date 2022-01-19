use std::rc::Rc;
use std::time::Duration;

mod toy;

async fn test_single_sleep(rt: Rc<toy::Runtime>) {
    println!("\ntest_single_sleep: single sleep event 1sec");
    toy::sleep(&rt, Duration::from_millis(1000)).await;
    println!("test_single_sleep: Done");
}

async fn test_single_nested(rt: Rc<toy::Runtime>) {
    println!("\ntest_single_nested: invoke nested_loop with 1sec sleep");
    rt.nested_loop(toy::sleep(&rt, Duration::from_millis(1000)));
    println!("test_single_nested: single sleep event 1sec");
}

async fn test_join_tree(rt: Rc<toy::Runtime>) {
    println!("\ntest_join_tree: run several task with nested loops");

    async fn task_tree1(rt: Rc<toy::Runtime>) {
        println!("task_tree1 started");
        toy::make_rt_join2(&rt, task_1(rt.clone()), task_2(rt.clone())).await;
        println!("task_tree1 done");
    }

    async fn task_tree2(rt: Rc<toy::Runtime>) {
        println!("task_tree2 started");
        toy::make_rt_join2(&rt, task_3(rt.clone()), task_4(rt.clone())).await;
        println!("task_tree2 done");
    }

    async fn task_1(rt: Rc<toy::Runtime>) {
        println!("task_1 started");
        toy::sleep(&rt, Duration::from_millis(1000)).await;
        println!("task_1 sleep done, starting nested loop");
        rt.nested_loop(toy::sleep(&rt, Duration::from_millis(1000)));
        println!("task_1 nested loop done");
    }

    async fn task_2(rt: Rc<toy::Runtime>) {
        println!("task_2 started");
        toy::sleep(&rt, Duration::from_millis(2000)).await;
        println!("task_2 done");
    }

    async fn task_3(rt: Rc<toy::Runtime>) {
        println!("task_3 started");
        toy::sleep(&rt, Duration::from_millis(3000)).await;
        println!("task_3 done");
    }

    async fn task_4(rt: Rc<toy::Runtime>) {
        println!("task_4 started");
        toy::sleep(&rt, Duration::from_millis(4000)).await;
        println!("task_4 sleep done, starting nested loop");
        rt.nested_loop(toy::sleep(&rt, Duration::from_millis(1000)));
        println!("task_4 nested loop done");
    }

    toy::make_rt_join2(&rt, task_tree1(rt.clone()), task_tree2(rt.clone())).await;
}

async fn test_nested_loop_tree(rt: Rc<toy::Runtime>) {
    println!("\ntest_nested_loop_tree: if we can have join and nested_loop in nested_loop");
    // just run the entire test_join_tree() in nested loop
    rt.nested_loop(test_join_tree(rt.clone()));
    println!("test_nested_loop_tree: done");
}

async fn test_frozen_events(rt: Rc<toy::Runtime>) {
    println!("\ntest_frozen_events: nested loops and frozen events");
    async fn task_a(rt: Rc<toy::Runtime>) {
        println!("task_a started");
        toy::sleep(&rt, Duration::from_millis(2000)).await;
        println!("task_a done");
    }

    async fn task_b(rt: Rc<toy::Runtime>) {
        println!("task_b sleep");
        toy::sleep(&rt, Duration::from_millis(1000)).await;
        println!("task_b nested loop");
        rt.nested_loop(toy::sleep(&rt, Duration::from_millis(2000)));
        println!("task_b done");
    }

    toy::make_join2(task_a(rt.clone()), task_b(rt.clone())).await;
    println!("test_frozen_events: done");
}

fn main() {
    toy::run(|rt| test_single_sleep(rt));
    toy::run(|rt| test_single_nested(rt));
    toy::run(|rt| test_join_tree(rt));
    toy::run(|rt| test_nested_loop_tree(rt));
    toy::run(|rt| test_frozen_events(rt));
}
