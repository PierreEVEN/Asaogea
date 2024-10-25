use anyhow::Error;
use job_sys::{Job, JobSystem};
use std::{thread, time};
use core::engine::Engine;
use core::options::WindowOptions;

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().init();
    let mut engine = Engine::new(WindowOptions { name: "Asaogea".to_string() })?;
    engine.run()
}

fn test() {
    let js = JobSystem::new(JobSystem::num_cpus() / 8);

    let mut handles = vec![];

    for i in 0..8 {
        handles.push(js.push(Job::new(move || {
            println!("start task {i}");
            thread::sleep(time::Duration::from_secs(2));
            format!("finished task {i}")
        })));
    }

    for handle in handles {
        let res = handle.wait();
        println!("{}", res.unwrap());
    }
}
