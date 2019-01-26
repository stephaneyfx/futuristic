// Copyright (C) 2018 Stephane Raux. Distributed under the MIT license.

//! Extensions to the `Sink` trait from the futures crate.

use either::Either;
use futures::Sink;

mod fork;
mod null;

pub use fork::Fork;
pub use null::Null;

/// Extension trait for `Sink`.
pub trait SinkTools: Sink {
    /// Returns a sink that dispatches to `self` or `other`.
    ///
    /// Every item sent to the returned sink is passed to `switch` and the
    /// returned value is sent to one of the underlying sinks. `Left` values
    /// are sent to `self` while `Right` values are sent to `other`.
    fn fork<T, O, F>(self, other: O, switch: F) -> Fork<T, Self, O, F>
    where
        Self: Sized,
        F: FnMut(T) -> Either<Self::SinkItem, O::SinkItem>,
        O: Sink<SinkError = Self::SinkError>,
    {
        Fork::new(self, other, switch)
    }
}

impl<S: Sink> SinkTools for S {}
