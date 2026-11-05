//! Read-only git is safe to run concurrently: a few threads at a time instead of one per item.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// `f` over every item on up to one thread per core, results in the items' order.
pub fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(items.len());
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(i) else { break };
                let result = f(item);
                done.lock().unwrap()[i] = Some(result);
            });
        }
    }); // a panicking worker panics here too: nothing is dropped silently
    done.into_inner().unwrap().into_iter().map(|r| r.expect("every item ran")).collect()
}
