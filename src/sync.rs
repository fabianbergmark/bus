pub(crate) use std::sync::Arc;
pub(crate) use std::sync::Weak;

#[cfg(loom)]
pub(crate) use loom::lazy_static;
#[cfg(loom)]
pub(crate) use loom::sync::Mutex;
#[cfg(loom)]
pub(crate) use loom::sync::MutexGuard;

#[cfg(not(loom))]
pub(crate) use lazy_static::lazy_static;
#[cfg(not(loom))]
pub(crate) use std::sync::Mutex;
#[cfg(not(loom))]
pub(crate) use std::sync::MutexGuard;
