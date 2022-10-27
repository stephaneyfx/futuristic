// Copyright (C) 2022 Stephane Raux. Distributed under the 0BSD license.

//! Tools for streams

use futures::Stream;

pub use zip_latest::ZipLatest;
pub use zip_latest_all::ZipLatestAll;
pub use zip_latest_with::ZipLatestWith;
pub use zip_latest_with_all::ZipLatestWithAll;

mod zip_latest;
mod zip_latest_all;
mod zip_latest_with;
mod zip_latest_with_all;

/// Extension trait for [`Stream`](futures::Stream).
pub trait StreamTools: Stream {
    /// Zips two streams using their latest values when one is not ready
    ///
    /// The zipped stream keeps the latest items produced by both streams. If one of the underlying
    /// streams is exhausted or not ready and the other stream yields a new item, it is combined
    /// with the latest item from the stream that did not yield anything new.
    ///
    /// The zipped stream ends when both underlying streams end, or if one of the streams ends
    /// without ever producing an item.
    ///
    /// Visually, this gives:
    /// ```text
    /// ---0-----------1-----------------2-------> self
    /// ------10-------11-------12---------------> other
    /// ------10-------12-------13-------14------> self.zip_latest_with(other, |a, b| a + b)
    /// ```
    fn zip_latest_with<S, F, T>(self, other: S, combine: F) -> ZipLatestWith<Self, S, F>
    where
        Self: Sized,
        S: Stream,
        F: FnMut(&Self::Item, &S::Item) -> T,
    {
        ZipLatestWith::new(self, other, combine)
    }

    /// Zips two streams using their latest values when one is not ready
    ///
    /// The zipped stream keeps a copy of the latest items produced by both streams. If one of the
    /// underlying streams is exhausted or not ready and the other stream yields a new item, it is
    /// returned alongside the latest item from the stream that did not yield anything new.
    ///
    /// The zipped stream ends when both underlying streams end, or if one of the streams ends
    /// without ever producing an item.
    ///
    /// Visually, this gives:
    /// ```text
    /// ---a-----------b-----------------c-------> self
    /// ------0--------1--------2----------------> other
    /// ------(a, 0)---(b, 1)---(b, 2)---(c, 2)--> self.zip_latest(other)
    /// ```
    fn zip_latest<S>(self, other: S) -> ZipLatest<Self, S>
    where
        Self: Sized,
        Self::Item: Clone,
        S: Stream,
        S::Item: Clone,
    {
        ZipLatest::new(self, other)
    }
}

impl<S: Stream> StreamTools for S {}

/// Zips multiple streams using their latest values for the ones that are not ready
///
/// The zipped stream keeps the latest items produced by all streams. If one of the underlying
/// streams is exhausted or not ready and at least one of the other streams yields a new item, it is
/// combined with the latest items from the streams that did not yield anything new.
///
/// The zipped stream ends when all underlying streams end, or if one of the streams ends
/// without ever producing an item.
///
/// Visually, this gives:
/// ```text
/// ---0-----------1-----------------2-------> a
/// ------10-------11-------12---------------> b
/// ------10-------12-------13-------14------> zip_latest_with_all([a, b], |a, b| a + b)
/// ```
pub fn zip_latest_with_all<I, F, T>(streams: I, combine: F) -> ZipLatestWithAll<I::Item, F>
where
    I: IntoIterator,
    I::Item: Stream + Unpin,
    F: FnMut(&[<I::Item as Stream>::Item]) -> T,
{
    ZipLatestWithAll::new(streams, combine)
}

/// Zips multiple streams using their latest values for the ones that are not ready
///
/// The zipped stream keeps a copy of the latest items produced by all streams. If one of the
/// underlying streams is exhausted or not ready and at least one of the other streams yields a new
/// item, it is returned alongside the latest items from the streams that did not yield anything
/// new.
///
/// The zipped stream ends when all underlying streams end, or if one of the streams ends
/// without ever producing an item.
///
/// Visually, this gives:
/// ```text
/// ---0--------------------1--------------------------> a
/// ------10----------------11-------------------------> b
/// ----------20--------------------------21-----------> c
/// ----------[0, 10, 20]---[1, 11, 20]---[1, 11, 21]--> zip_latest_all([a, b, c])
/// ```
pub fn zip_latest_all<I>(streams: I) -> ZipLatestAll<I::Item>
where
    I: IntoIterator,
    I::Item: Stream + Unpin,
    <I::Item as Stream>::Item: Clone,
{
    ZipLatestAll::new(streams)
}

#[cfg(test)]
mod test_util {
    use crate::future::yield_now;
    use futures::{Stream, StreamExt};

    pub fn yield_on_none<I, T>(items: I) -> impl Stream<Item = T>
    where
        I: IntoIterator<Item = Option<T>>,
    {
        futures::stream::iter(items).filter_map(|x| async move {
            if x.is_none() {
                yield_now().await;
            }
            x
        })
    }
}
