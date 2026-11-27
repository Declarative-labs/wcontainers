// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! The objects Mesa holds, by key: each map of one kind under its own lock, held only while a key is looked up,
//! inserted or taken out, never while code of either side runs (a call clones the `Arc` it needs and lets the lock
//! go). Keys are numbers the glue hands Mesa (a state's handle is its key); a key a map does not hold refuses the call.
//! Every screen being served is in `SCREENS` by its key, so the C trampolines reach it by number alone.

use std::any::Any;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// The objects of one kind, by key.
pub struct Map<T> {
    objects: Mutex<HashMap<usize, Arc<T>>>,
}

impl<T> Default for Map<T> {
    fn default() -> Self {
        Map { objects: Mutex::new(HashMap::new()) }
    }
}

/// The next key handed out: never zero (zero names no object), never handed out twice.
static NEXT_KEY: AtomicUsize = AtomicUsize::new(1);

/// A new key.
pub fn new_key() -> usize {
    NEXT_KEY.fetch_add(1, Ordering::Relaxed)
}

impl<T> Map<T> {
    /// Keeps an object made by the implementation; its new key.
    pub fn insert(&self, object: T) -> usize {
        let key = new_key();
        self.objects.lock().unwrap_or_else(PoisonError::into_inner).insert(key, Arc::new(object));
        key
    }

    /// The object of a key, for the length of a call; None for a key not held.
    pub fn get(&self, key: usize) -> Option<Arc<T>> {
        self.objects.lock().unwrap_or_else(PoisonError::into_inner).get(&key).cloned()
    }

    /// The object of a key taken out, to be given back to the implementation by value; None for a key not held, and
    /// for an object another call holds at this moment (it stays, and the call is refused).
    pub fn take(&self, key: usize) -> Option<T> {
        let taken = self.objects.lock().unwrap_or_else(PoisonError::into_inner).remove(&key)?;
        match Arc::try_unwrap(taken) {
            Ok(object) => Some(object),
            Err(shared) => {
                self.objects.lock().unwrap_or_else(PoisonError::into_inner).insert(key, shared);
                None
            }
        }
    }

    /// How many objects are held.
    pub fn len(&self) -> usize {
        self.objects.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Whether none is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Every screen being served, by key, each a `Served<G>` of its implementation's type.
static SCREENS: Mutex<Vec<(usize, Arc<dyn Any + Send + Sync>)>> = Mutex::new(Vec::new());

/// Keeps a screen being served; its key.
pub fn keep_screen<S: Any + Send + Sync>(served: Arc<S>) -> usize {
    let key = new_key();
    SCREENS.lock().unwrap_or_else(PoisonError::into_inner).push((key, served));
    key
}

/// The screen of a key, if it is one being served of this type.
pub fn screen<S: Any + Send + Sync>(key: usize) -> Option<Arc<S>> {
    let found = SCREENS.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(held, _)| *held == key).map(|(_, served)| Arc::clone(served))?;
    found.downcast::<S>().ok()
}

/// The screen of a key taken out (its end); None where it is not one being served of this type.
pub fn take_screen<S: Any + Send + Sync>(key: usize) -> Option<Arc<S>> {
    let mut screens = SCREENS.lock().unwrap_or_else(PoisonError::into_inner);
    let at = screens.iter().position(|(held, served)| *held == key && served.is::<S>())?;
    let (_, served) = screens.remove(at);
    drop(screens);
    served.downcast::<S>().ok()
}
