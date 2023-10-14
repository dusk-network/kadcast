// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.
use std::{sync::Arc, time::Duration};

use tokio::sync::RwLock as ExtRwLock;
#[cfg(feature = "diagnostics")]
use tokio::{
    sync::{RwLockReadGuard, RwLockWriteGuard},
    time::timeout,
};
#[cfg(feature = "diagnostics")]
use tracing::warn;

#[cfg(feature = "diagnostics")]
pub(super) type RwLock<T> = DiagnosticRwLock<T>;

#[cfg(feature = "diagnostics")]
pub(super) struct DiagnosticRwLock<T> {
    arc_lock: Arc<ExtRwLock<T>>,
    timeout: Duration,
}

#[cfg(not(feature = "diagnostics"))]
pub(super) type RwLock<T> = Arc<ExtRwLock<T>>;

#[cfg(feature = "diagnostics")]
pub(super) fn new_rwlock<T>(value: T, timeout: Duration) -> RwLock<T> {
    return DiagnosticRwLock::new(value, timeout);
}

#[cfg(not(feature = "diagnostics"))]
pub(super) fn new_rwlock<T>(value: T, _timeout: Duration) -> RwLock<T> {
    return Arc::new(ExtRwLock::new(value));
}

#[cfg(feature = "diagnostics")]
impl<T> DiagnosticRwLock<T> {
    pub(crate) fn new(inner: T, timeout: Duration) -> Self {
        Self {
            arc_lock: Arc::new(ExtRwLock::new(inner)),
            timeout,
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

#[cfg(feature = "diagnostics")]
impl<T> Clone for DiagnosticRwLock<T> {
    fn clone(&self) -> Self {
        Self {
            arc_lock: self.arc_lock.clone(),
            timeout: self.timeout,
        }
    }
}
