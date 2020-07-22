use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam::queue::{ArrayQueue, PushError};
use crossbeam::utils::Backoff;

use crate::cell::{AtomicRefCell, AtomicRefMut};
use crate::system::executor::SystemExecutor;
use crate::system::System;
use crate::utils;
use crate::world::World;

#[derive(Debug)]
pub struct DispatchBuilder {
    thread_count: Option<usize>,
    sleep_time: Option<Duration>,
    systems: Vec<SystemExecutor>,
}

impl DispatchBuilder {
    pub fn new() -> Self {
        Self {
            thread_count: None,
            sleep_time: None,
            systems: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            thread_count: None,
            sleep_time: None,
            systems: Vec::with_capacity(capacity),
        }
    }

    pub fn with_system<S>(mut self, system: S) -> Self
    where
        S: for<'a> System<'a> + Send + Sync,
    {
        let executor = SystemExecutor::new(system);
        self.systems.push(executor);
        self
    }

    pub fn with_threads(mut self, thread_count: usize) -> Self {
        self.thread_count = Some(thread_count);
        self
    }

    pub fn with_sleep(mut self, sleep_time: Duration) -> Self {
        self.sleep_time = Some(sleep_time);
        self
    }

    pub fn build(mut self, world: World) -> Dispatcher {
        let dispatcher = Dispatcher::new_priv(
            world,
            self.systems.len(),
            self.thread_count,
            self.sleep_time,
        );

        self.systems.drain(..).for_each(|executor| {
            dispatcher
                .shared
                .queue
                .push_no_cache(executor)
                .unwrap_or_else(|_| unsafe {
                    utils::debug_unreachable("Incorrect Dispatcher capacity.")
                })
        });

        dispatcher
    }
}

#[derive(Debug)]
pub struct Dispatcher {
    threads: Vec<DispatchThread>,
    shared: Arc<ThreadShared>,
}

impl Dispatcher {
    pub fn new(world: World, capacity: usize) -> Self {
        Self::new_priv(world, capacity, None, None)
    }

    pub fn with_threads(world: World, capacity: usize, thread_count: usize) -> Self {
        Self::new_priv(world, capacity, Some(thread_count), None)
    }

    pub fn with_sleep(world: World, capacity: usize, sleep_time: Duration) -> Self {
        Self::new_priv(world, capacity, None, Some(sleep_time))
    }

    pub fn with_threads_and_sleep(
        world: World,
        capacity: usize,
        thread_count: usize,
        sleep_time: Duration,
    ) -> Self {
        Self::new_priv(world, capacity, Some(thread_count), Some(sleep_time))
    }

    pub fn add_executor(&self, executor: SystemExecutor) -> Result<(), SystemExecutor> {
        self.shared.queue.push(executor)
    }

    pub fn world(&self) -> WorldHandle {
        self.park_all();

        let backoff = Backoff::new();
        while self.shared.parked.load(Ordering::Acquire) != self.threads.len() {
            if backoff.is_completed() {
                thread::sleep(Duration::from_millis(1));
            } else {
                backoff.snooze();
            }
        }

        WorldHandle(self, self.shared.world.borrow_mut())
    }

    pub fn shutdown(mut self) -> World {
        self.unpark_all();
        self.shared.status.store(SHUTDOWN, Ordering::Relaxed);

        self.threads.drain(..).for_each(|thread| thread.join());

        let shared = Arc::try_unwrap(self.shared).expect("Not all dispatcher threads were joined.");

        shared.world.into_inner()
    }

    pub fn dispatch(&mut self) {
        let thread_count = self.threads.capacity();

        for id in 0..thread_count {
            self.threads.push(DispatchThread::spawn(
                format!("Dispatcher thread #{}", id),
                &self.shared,
            ));
        }
    }

    fn new_priv(
        world: World,
        capacity: usize,
        thread_count: Option<usize>,
        sleep_time: Option<Duration>,
    ) -> Self {
        let count = match thread_count {
            Some(n) => n,
            None => num_cpus::get(),
        };

        let shared = Arc::new(ThreadShared::new(
            SystemQueue::new(capacity),
            world,
            sleep_time,
        ));

        Self {
            threads: Vec::with_capacity(count),
            shared,
        }
    }

    fn park_all(&self) {
        self.shared.status.store(PARKED, Ordering::Release);
    }

    fn unpark_all(&self) {
        self.threads.iter().for_each(|thread| thread.unpark());
    }
}

#[derive(Debug)]
pub struct SystemQueue {
    cache: AtomicRefCell<Option<SystemExecutor>>,
    queue: ArrayQueue<SystemExecutor>,
}

impl SystemQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: AtomicRefCell::new(None),
            queue: ArrayQueue::new(capacity),
        }
    }

    pub fn push(&self, executor: SystemExecutor) -> Result<(), SystemExecutor> {
        match self.cache.try_borrow_mut() {
            Some(mut cached) if cached.is_none() => {
                *cached = Some(executor);
                Ok(())
            }
            _ => self
                .queue
                .push(executor)
                .map_err(|PushError(executor)| executor),
        }
    }

    pub fn push_no_cache(&self, executor: SystemExecutor) -> Result<(), SystemExecutor> {
        self.queue
            .push(executor)
            .map_err(|PushError(executor)| executor)
    }

    pub fn pop(&self) -> Option<SystemExecutor> {
        match self.cache.try_borrow_mut() {
            Some(mut cached) if cached.is_some() => cached.take(),
            _ => self.queue.pop().ok(),
        }
    }
}

const RUNNING: usize = 0;
const PARKED: usize = 1;
const SHUTDOWN: usize = 2;

#[derive(Debug)]
struct DispatchThread {
    join_handle: thread::JoinHandle<()>,
}

impl DispatchThread {
    fn spawn(name: String, shared: &Arc<ThreadShared>) -> Self {
        let shared = Arc::clone(shared);
        let join_handle = thread::Builder::new()
            .name(name)
            .spawn(Self::thread_closure(shared))
            .expect("Unable to spawn Dispatch thread.");

        Self { join_handle }
    }

    fn unpark(&self) {
        self.join_handle.thread().unpark();
    }

    fn join(self) {
        let thread = self.join_handle.thread().clone();
        let thread_name = thread.name().unwrap();

        self.join_handle.join().unwrap_or_else(|e| {
            let msg = if let Some(msg) = e.downcast_ref::<String>() {
                format!("thread {} panicked at `{}`", thread_name, msg)
            } else if let Some(&msg) = e.downcast_ref::<&str>() {
                format!("thread {} panicked at `{}`", thread_name, msg)
            } else {
                format!("thread {} panicked at `Box<dyn Any + Send>`", thread_name)
            };

            panic!(msg);
        })
    }

    fn thread_closure(shared: Arc<ThreadShared>) -> impl FnOnce() + Send {
        move || {
            let backoff = Backoff::new();

            loop {
                match shared.status.load(Ordering::Acquire) {
                    RUNNING => {
                        shared.execute(&backoff, shared.sleep_time);
                    }
                    PARKED => {
                        shared.parked.fetch_add(1, Ordering::AcqRel);
                        thread::park();
                        shared.parked.fetch_sub(1, Ordering::AcqRel);
                    }
                    SHUTDOWN => {
                        return;
                    }
                    _ => unsafe {
                        utils::debug_unreachable("Inconsistent thread state.");
                    },
                }
            }
        }
    }
}

#[derive(Debug)]
struct ThreadShared {
    status: AtomicUsize,
    parked: AtomicUsize,
    world: AtomicRefCell<World>,
    queue: SystemQueue,
    sleep_time: Option<Duration>,
}

impl ThreadShared {
    fn new(queue: SystemQueue, world: World, sleep_time: Option<Duration>) -> Self {
        Self {
            status: AtomicUsize::new(RUNNING),
            parked: AtomicUsize::new(0),
            world: AtomicRefCell::new(world),
            queue,
            sleep_time,
        }
    }

    fn execute(&self, backoff: &Backoff, sleep_time: Option<Duration>) {
        let world = self.world.borrow();

        let resource_storage = world.resource_storage();
        let component_storage = world.component_storage();

        match self.queue.pop() {
            Some(mut executor) => {
                if let Some(time) = sleep_time {
                    thread::sleep(time);
                }

                // FIXME: Do something with this Result.
                executor.execute(resource_storage, component_storage).ok();
                self.queue.push(executor).expect("System queue was full.");

                backoff.reset();
            }
            None => {
                if backoff.is_completed() {
                    // Reduces CPU usage, but also doesn't introduce
                    // too much latency, since systems are usually
                    // run constantly.
                    thread::sleep(Duration::from_millis(1));
                }

                backoff.snooze();
            }
        }
    }
}

#[derive(Debug)]
pub struct WorldHandle<'a>(&'a Dispatcher, AtomicRefMut<'a, World>);

impl Deref for WorldHandle<'_> {
    type Target = World;

    fn deref(&self) -> &World {
        &self.1
    }
}

impl DerefMut for WorldHandle<'_> {
    fn deref_mut(&mut self) -> &mut World {
        &mut self.1
    }
}

impl Drop for WorldHandle<'_> {
    fn drop(&mut self) {
        self.0.unpark_all()
    }
}
