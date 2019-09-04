// Copyright (C) 2018 Stephane Raux. Distributed under the MIT license.

//! Extensions to the `Sink` trait from the futures crate.

use either::Either;
use futures::Sink;

mod fork;

pub use fork::Fork;

/// Extension trait for `Sink`.
pub trait SinkTools<T>: Sink<T> {
    /// Returns a sink that dispatches to `self` or `other`.
    ///
    /// Every item sent to the returned sink is passed to `switch` and the returned value is sent
    /// to one of the underlying sinks. `Left` values are sent to `self` while `Right` values are
    /// sent to `other`.
    fn fork<V, O, F, U>(self, other: O, switch: F) -> Fork<V, Self, O, F, T, U>
    where
        Self: Sized,
        F: FnMut(V) -> Either<T, U>,
        O: Sink<U, Error = Self::Error>,
    {
        Fork::new(self, other, switch)
    }
}

impl<T, S: Sink<T>> SinkTools<T> for S {}
