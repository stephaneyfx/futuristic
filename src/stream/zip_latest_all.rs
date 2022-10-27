// Copyright (C) 2022 Stephane Raux. Distributed under the 0BSD license.

use crate::stream::ZipLatestWithAll;
use futures::{stream::FusedStream, Stream};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// Stream returned by [`zip_latest_all`](crate::stream::zip_latest_all).
pub struct ZipLatestAll<S>(ZipLatestWithAll<S, fn(&[S::Item]) -> Vec<S::Item>>)
where
    S: Stream + Unpin;

impl<S> ZipLatestAll<S>
where
    S: Stream + Unpin,
    S::Item: Clone,
{
    pub(crate) fn new<I>(streams: I) -> Self
    where
        I: IntoIterator<Item = S>,
    {
        Self(ZipLatestWithAll::new(streams, |items| items.to_vec()))
    }
}

impl<S> Stream for ZipLatestAll<S>
where
    S: Stream + Unpin,
    S::Item: Clone,
{
    type Item = Vec<S::Item>;

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.0).poll_next(ctx)
    }
}

impl<S> FusedStream for ZipLatestAll<S>
where
    S: Stream + Unpin,
    S::Item: Clone,
{
    fn is_terminated(&self) -> bool {
        self.0.is_terminated()
    }
}

#[cfg(test)]
mod tests {
    use crate::stream::{test_util::yield_on_none, zip_latest_all};
    use futures::{
        executor::block_on,
        pin_mut,
        stream::{empty, repeat},
        StreamExt,
    };

    #[test]
    fn it_works() {
        let a = yield_on_none([Some(0), None, Some(1), None, None, Some(2)]);
        pin_mut!(a);
        let b = yield_on_none([None, Some(10), Some(11), Some(12), None, None, Some(13)]);
        pin_mut!(b);
        let expected = [
            vec![0, 10],
            vec![1, 11],
            vec![1, 12],
            vec![2, 12],
            vec![2, 13],
        ];
        let actual =
            block_on(zip_latest_all([a.left_stream(), b.right_stream()]).collect::<Vec<_>>());
        assert_eq!(actual, expected);
    }

    #[test]
    fn zipping_latest_of_2_empty_streams_gives_empty_stream() {
        let r = block_on(zip_latest_all([empty::<()>(), empty()]).collect::<Vec<_>>());
        assert_eq!(r, <[Vec<()>; 0]>::default());
    }

    #[test]
    fn zipping_latest_of_empty_and_infinite_streams_gives_empty_stream() {
        let r = block_on(
            zip_latest_all([empty::<()>().left_stream(), repeat(()).right_stream()])
                .collect::<Vec<_>>(),
        );
        assert_eq!(r, <[Vec<()>; 0]>::default());
    }
}
