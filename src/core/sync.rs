//! Recovery helpers for rebuildable process caches.
//!
//! These helpers are intentionally limited to `RwLock`: callers may recover
//! only when the protected value is derived/cache state whose invariants can
//! be rebuilt after a panic. Stateful transaction locks must handle poisoning
//! explicitly instead.

#[cfg(any(
    test,
    feature = "debian",
    feature = "debian-pure",
    feature = "macos",
    target_os = "macos"
))]
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Acquire a read guard, recovering rebuildable cache state after poisoning.
#[cfg(any(
    test,
    feature = "debian",
    feature = "debian-pure",
    feature = "macos",
    target_os = "macos"
))]
pub(crate) fn read_cache<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Acquire a write guard, recovering rebuildable cache state after poisoning.
#[cfg(any(
    test,
    feature = "debian",
    feature = "debian-pure",
    feature = "macos",
    target_os = "macos"
))]
pub(crate) fn write_cache<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn rebuildable_cache_remains_accessible_after_writer_panic() {
        let cache = Arc::new(RwLock::new(vec![1_u8]));
        let poisoned = Arc::clone(&cache);
        let _ = std::thread::spawn(move || {
            let mut guard = poisoned.write().expect("initial lock");
            guard.push(2);
            panic!("poison cache");
        })
        .join();

        assert_eq!(&*read_cache(&cache), &[1, 2]);
        write_cache(&cache).clear();
        assert!(read_cache(&cache).is_empty());
    }
}
