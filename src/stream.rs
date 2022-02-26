// Copyright (C) 2022 Stephane Raux. Distributed under the 0BSD license.

//! Extensions to the [`Stream`] trait from the [`futures`] crate.

use futures::Stream;

pub use zip_latest::ZipLatest;

mod zip_latest;

/// Extension trait for [`Stream`].
pub trait StreamTools: Stream {
    /// Zips two streams using their latest values when one is not ready
    ///
    /// The zipped stream keeps a copy of the latest items produced for both streams. If one of the
    /// underlying streams is exhausted or not ready and the other stream yields a new item, it is
    /// returned alongside the latest item from the stream that did not yield anything.
    ///
    /// The zipped stream ends when both underlying streams end, or if one of the streams ends
    /// without ever producing an item.
    ///
    /// Visually this gives:
    /// ```text
    /// ---a-----------b-----------------c-------> self
    /// ------0--------1--------2----------------> other
    /// ------(a, 0)---(b, 1)---(b, 2)---(c, 2)--> self.zip_latest(other)
    fn zip_latest<S>(self, other: S) -> ZipLatest<Self, S>
    where
        Self: Sized,
        S: Stream,
    {
        ZipLatest::new(self, other)
    }
}

impl<S: Stream> StreamTools for S {}
