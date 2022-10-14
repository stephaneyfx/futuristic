// Copyright (C) 2022 Stephane Raux. Distributed under the 0BSD license.

use futures::{
    stream::{Fuse, FusedStream},
    Stream, StreamExt,
};
use pin_project::pin_project;
use std::{
    mem,
    pin::Pin,
    task::{Context, Poll},
};

/// Stream returned by [`StreamTools::zip_latest`](crate::StreamTools::zip_latest).
#[pin_project]
#[derive(Debug)]
pub struct ZipLatest<A, B>
where
    A: Stream,
    B: Stream,
{
    #[pin]
    stream: Fuse<A>,
    #[pin]
    other_stream: Fuse<B>,
    state: StreamState<A::Item>,
    other_state: StreamState<B::Item>,
}

impl<A, B> ZipLatest<A, B>
where
    A: Stream,
    B: Stream,
{
    pub(crate) fn new(stream: A, other_stream: B) -> Self {
        Self {
            stream: stream.fuse(),
            other_stream: other_stream.fuse(),
            state: StreamState::Nothing,
            other_state: StreamState::Nothing,
        }
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
        let mut this = self.project();
        if this.state.needs_poll() {
            if let Poll::Ready(Some(x)) = this.stream.as_mut().poll_next(ctx) {
                *this.state = StreamState::New(x);
            }
        }
        if this.other_state.needs_poll() {
            if let Poll::Ready(Some(x)) = this.other_stream.as_mut().poll_next(ctx) {
                *this.other_state = StreamState::New(x);
            }
        }
        let (new_state, new_other_state, res) = match (
            mem::replace(this.state, StreamState::Nothing),
            mem::replace(this.other_state, StreamState::Nothing),
        ) {
            (StreamState::New(a), StreamState::New(b))
            | (StreamState::New(a), StreamState::Yielded(b))
            | (StreamState::Yielded(a), StreamState::New(b)) => (
                StreamState::Yielded(a.clone()),
                StreamState::Yielded(b.clone()),
                Poll::Ready(Some((a, b))),
            ),
            (StreamState::Nothing, _) if this.stream.is_done() => (
                StreamState::Nothing,
                StreamState::Nothing,
                Poll::Ready(None),
            ),
            (_, StreamState::Nothing) if this.other_stream.is_done() => (
                StreamState::Nothing,
                StreamState::Nothing,
                Poll::Ready(None),
            ),
            _ if this.stream.is_done() && this.other_stream.is_done() => (
                StreamState::Nothing,
                StreamState::Nothing,
                Poll::Ready(None),
            ),
            (a, b) => (a, b, Poll::Pending),
        };
        *this.state = new_state;
        *this.other_state = new_other_state;
        res
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
        matches!(
            (&self.state, self.stream.is_done()),
            (StreamState::Nothing, true)
        ) || matches!(
            (&self.other_state, self.other_stream.is_done()),
            (StreamState::Nothing, true)
        )
    }
}

#[derive(Debug)]
enum StreamState<T> {
    Nothing,
    New(T),
    Yielded(T),
}

impl<T> StreamState<T> {
    fn needs_poll(&self) -> bool {
        match self {
            StreamState::Nothing | StreamState::Yielded(_) => true,
            StreamState::New(_) => false,
        }
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
