use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::thread::{JoinHandle};
use anyhow::{anyhow, Error};
use lockfree::prelude::Queue;

pub struct JobSystem {
    _workers: Vec<Worker>,
    job_pool: Arc<JobPool>,
}

pub struct JobPool {
    job_pool: Queue<Option<Box<dyn Task>>>,
    job_semaphore: Mutex<usize>,
    job_semaphore_cond: Condvar,
}

impl Default for JobPool {
    fn default() -> Self {
        let pool = Queue::new();

        Self {
            job_pool: pool,
            job_semaphore: Mutex::new(0),
            job_semaphore_cond: Default::default(),
        }
    }
}

impl JobPool {
    pub fn push<R: Sized, T: 'static + FnOnce() -> R>(&self, job: Job<R, T>) -> JobHandle<R> {
        let ret = job.ret.clone();
        self.job_pool.push(Some(Box::new(job)));
        *(self.job_semaphore.lock().unwrap()) += 1;
        self.job_semaphore_cond.notify_one();
        JobHandle { return_value: ret }
    }

    pub fn pop(&self) -> Result<Box<dyn Task>, Error> {
        let mut guard = self.job_semaphore.lock().unwrap();
        while *guard == 0 {
            guard = self.job_semaphore_cond.wait(guard).unwrap();
        }
        *guard -= 1;
        Ok(self.job_pool.pop().ok_or(anyhow!("Out of task"))?.unwrap())
    }

    fn free(&self, num_threads: usize) {
        for _ in 0..num_threads {
            *(self.job_semaphore.lock().unwrap()) += 1;
            self.job_semaphore_cond.notify_one();
        }
    }
}

impl JobSystem {
    pub fn num_cpus() -> usize {
        num_cpus::get()
    }

    pub fn new(job_count: usize) -> Self {
        let mut workers = vec![];
        let job_pool = Arc::new(JobPool::default());

        for _ in 0..job_count {
            workers.push(Worker::new(job_pool.clone()));
        }

        Self {
            _workers: workers,
            job_pool,
        }
    }

    pub fn push<R: Sized, T: 'static + FnOnce() -> R>(&self, job: Job<R, T>) -> JobHandle<R> {
        self.job_pool.push(job)
    }
}

impl Drop for JobSystem {
    fn drop(&mut self) {
        self.job_pool.free(self._workers.len());
    }
}

pub struct Worker {
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn new(job_pool: Arc<JobPool>) -> Self {
        let test = thread::spawn(move || {
            loop {
                let task = job_pool.pop().map_err(|e| { anyhow!("Worker failed to acquire task : {e}") });
                match task {
                    Ok(mut task) => {
                        task.execute()
                    }
                    Err(err) => {
                        println!("{err}");
                        return;
                    }
                }
            }
        });

        Self {
            thread: Some(test),
        }
    }
}


impl Drop for Worker {
    fn drop(&mut self) {
        self.thread.take().unwrap().join().expect("Failed to wait for worker");
    }
}

pub trait Task: Send + Sync {
    fn execute(&mut self);
}

pub struct Job<R: Sized, T: FnOnce() -> R> {
    callback: Option<T>,
    ret: Arc<(Mutex<Option<R>>, Condvar)>,
}

unsafe impl<R: Sized, T: FnOnce() -> R> Send for Job<R, T> {}
unsafe impl<R: Sized, T: FnOnce() -> R> Sync for Job<R, T> {}

impl<R: Sized, T: FnOnce() -> R> Job<R, T> {
    pub fn new(callback: T) -> Self {
        Self {
            callback: Some(callback),
            ret: Arc::new((Mutex::new(None), Condvar::new())),
        }
    }
}

impl<R: Sized, T: FnOnce() -> R> Task for Job<R, T> {
    fn execute(&mut self) {
        if let Some(callback) = self.callback.take() {
            let (ret, cond) = self.ret.as_ref();
            let result = callback();
            let mut lock = ret.lock().unwrap();
            *lock = Some(result);
            cond.notify_all();
        }
    }
}

#[derive(Clone)]
pub struct JobHandle<R: Sized + 'static> {
    return_value: Arc<(Mutex<Option<R>>, Condvar)>,
}

impl<R: Sized> JobHandle<R> {
    pub fn wait(self) -> Option<R> {
        let (ret, cond) = self.return_value.as_ref();

        let mut guard = ret.lock().unwrap();
        while guard.is_none() {
            guard = cond.wait(guard).unwrap();
        }
        guard.take()
    }

    pub fn get_ref(&self) -> MutexGuard<Option<R>> {
        self.return_value.as_ref().0.lock().unwrap()
    }
}