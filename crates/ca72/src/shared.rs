//! Tables every voice would otherwise make a copy of, made once a process and shared
//! (performance: a voice carried about 400 KB of tables that every voice has the same of,
//! which crowded the caches when many play).

use std::sync::{Arc, Mutex, PoisonError};

/// A process's tables of one kind by the key they were made for.
#[derive(Debug)]
pub struct Shared<K, T> {
    made: Mutex<Vec<(K, Arc<T>)>>,
}

impl<K: PartialEq, T> Shared<K, T> {
    pub const fn new() -> Self {
        Shared {
            made: Mutex::new(Vec::new()),
        }
    }

    /// The table made for an equal key, or `make`'s, kept for the next (not on the audio
    /// thread: it can make one, and it locks).
    pub fn get(&self, key: K, make: impl FnOnce() -> T) -> Arc<T> {
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, t)) = made.iter().find(|(k, _)| *k == key) {
            return t.clone();
        }
        let t = Arc::new(make());
        made.push((key, t.clone()));
        t
    }
}

impl<K: PartialEq, T> Default for Shared<K, T> {
    fn default() -> Self {
        Self::new()
    }
}
