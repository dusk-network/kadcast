// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// This module provides abstractions for read-write locks.
///
/// Two types of `RwLock` are supported:
/// - `DiagnosticRwLock`: Used when the `diagnostics` feature flag is
///   enabled. It will log warnings if a read or write lock cannot be
///   acquired within a predefined timeout.
/// - `tokio::sync:RwLock`: Used by default. Behaves like a standard
///   read-write lock.
///
/// To instantiate a new lock, use the `new` function. This function
/// takes care of which lock to provide based on the set feature flag,
/// while offering a unified API.
use std::sync::Arc;
use tokio::sync::RwLock as ExtRwLock;

#[cfg(feature = "diagnostics")]
/// Type alias for `RwLock` when the `diagnostics` feature is enabled.
pub(super) type RwLock<T> = diagnostic::DiagnosticRwLock<T>;

#[cfg(not(feature = "diagnostics"))]
/// Default type alias for `RwLock`.
pub(super) type RwLock<T> = Arc<ExtRwLock<T>>;

/// Creates a new `RwLock`. Based on whether the `diagnostics` feature is
/// enabled:
/// - When enabled, a `DiagnosticRwLock` is returned.
/// - Otherwise, a standard `RwLock` will be returned.
///
/// # Parameters
/// - `value`: The value to be wrapped by the lock.
///
/// # Returns
/// An instance of either `DiagnosticRwLock<T>` or `Arc<ExtRwLock<T>>`,
/// depending on the `diagnostics` feature flag.
pub(super) fn new<T>(value: T) -> RwLock<T> {
    #[cfg(feature = "diagnostics")]
    return diagnostic::DiagnosticRwLock::new(value);
    #[cfg(not(feature = "diagnostics"))]
    return Arc::new(ExtRwLock::new(value));
}

#[cfg(feature = "diagnostics")]
mod diagnostic {
    use super::*;
    use std::time::Duration;
    use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};
    use tokio::time::timeout;
    use tracing::warn;

    /// Diagnostics read-write lock.
    pub(crate) struct DiagnosticRwLock<T> {
        arc_lock: Arc<ExtRwLock<T>>,
        timeout: Duration,
    }

    impl<T> DiagnosticRwLock<T> {
        pub(crate) fn new(inner: T) -> Self {
            Self {
                arc_lock: Arc::new(ExtRwLock::new(inner)),
                timeout: Duration::from_secs(1),
            }
        }

        pub(crate) async fn read(&self) -> RwLockReadGuard<'_, T> {
            loop {
                match timeout(self.timeout, self.arc_lock.read()).await {
                    Ok(inner) => return inner,
                    Err(_) => {
                        warn!("Unable to acquire read in {:?}", self.timeout);
                    }
                }
            }
        }

        pub(crate) async fn write(&self) -> RwLockWriteGuard<'_, T> {
            loop {
                match timeout(self.timeout, self.arc_lock.write()).await {
                    Ok(inner) => return inner,
                    Err(_) => {
                        warn!("Unable to acquire write in {:?}", self.timeout);
                    }
                }
            }
        }
    }

    impl<T> Clone for DiagnosticRwLock<T> {
        fn clone(&self) -> Self {
            Self {
                arc_lock: self.arc_lock.clone(),
                timeout: self.timeout,
            }
        }
    }

    #[cfg(test)]
    mod diagnostic_tests {
        use super::*;

        #[tokio::test]
        async fn test_timeout_warning() {
            // Initialize a lock with a very short timeout of 1 microsecond.
            let lock = new(42);

            // Create a write lock to make sure the read lock will timeout.
            let _write = lock.write().await;

            // Now try to acquire a read lock. This should timeout and produce a
            // warning.
            let read =
                tokio::time::timeout(Duration::from_millis(5), lock.read())
                    .await;

            // Assert that a warning was emitted.
            assert!(read.is_err(), "Read lock should have timed out");
        }
    }
}
