// Copyright (C) 2022 Stephane Raux. Distributed under the 0BSD license.

use crate::stream::ZipLatestWith;
use futures::{stream::FusedStream, Stream};
use pin_project::pin_project;
use std::{
    fmt::{self, Debug},
    pin::Pin,
    task::{Context, Poll},
};

/// Stream returned by [`StreamTools::zip_latest`](crate::StreamTools::zip_latest).
#[pin_project]
pub struct ZipLatest<A, B>(
    #[pin] ZipLatestWith<A, B, fn(&A::Item, &B::Item) -> (A::Item, B::Item)>,
)
where
    A: Stream,
    B: Stream;

impl<A, B> ZipLatest<A, B>
where
    A: Stream,
    A::Item: Clone,
    B: Stream,
    B::Item: Clone,
{
    pub(crate) fn new(stream: A, other_stream: B) -> Self {
        Self(ZipLatestWith::new(stream, other_stream, |a, b| {
            (a.clone(), b.clone())
        }))
    }
}

impl<A, B> Debug for ZipLatest<A, B>
where
    A: Stream,
    B: Stream,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ZipLatest")
    }
}

impl<A, B> Stream for ZipLatest<A, B>
where
    A: Stream,
    A::Item: Clone,
    B: Stream,
    B::Item: Clone,
{
    type Item = (A::Item, B::Item);

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().0.poll_next(ctx)
    }
}

impl<A, B> FusedStream for ZipLatest<A, B>
where
    A: Stream,
    A::Item: Clone,
    B: Stream,
    B::Item: Clone,
{
    fn is_terminated(&self) -> bool {
        self.0.is_terminated()
    }
}

#[cfg(test)]
mod tests {
    use crate::{stream::test_util::yield_on_none, StreamTools};
    use futures::{
        executor::block_on,
        stream::{empty, repeat},
        StreamExt,
    };

    #[test]
    fn it_works() {
        let a = yield_on_none([Some(0), None, Some(1), None, None, Some(2)]);
        let b = yield_on_none([None, Some(10), Some(11), Some(12), None, None, Some(13)]);
        let expected = [(0, 10), (0, 11), (1, 12), (2, 13)];
        let actual = block_on(a.zip_latest(b).collect::<Vec<_>>());
        assert_eq!(actual, expected);
    }

    #[test]
    fn zipping_latest_of_2_empty_streams_gives_empty_stream() {
        let r = block_on(empty::<()>().zip_latest(empty::<()>()).collect::<Vec<_>>());
        assert_eq!(r, []);
    }

    #[test]
    fn zipping_latest_of_empty_and_infinite_streams_gives_empty_stream() {
        let r = block_on(empty::<()>().zip_latest(repeat(())).collect::<Vec<_>>());
        assert_eq!(r, []);
    }
}
