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

/// Stream returned by [`StreamTools::zip_latest_with`](crate::StreamTools::zip_latest_with).
#[pin_project]
#[derive(Debug)]
pub struct ZipLatestWith<A, B, F>
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
    combine: F,
}

impl<A, B, F, T> ZipLatestWith<A, B, F>
where
    A: Stream,
    B: Stream,
    F: FnMut(&A::Item, &B::Item) -> T,
{
    pub(crate) fn new(stream: A, other_stream: B, combine: F) -> Self {
        Self {
            stream: stream.fuse(),
            other_stream: other_stream.fuse(),
            state: StreamState::Nothing,
            other_state: StreamState::Nothing,
            combine,
        }
    }
}

impl<A, B, F, T> Stream for ZipLatestWith<A, B, F>
where
    A: Stream,
    B: Stream,
    F: FnMut(&A::Item, &B::Item) -> T,
{
    type Item = T;

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
        let (res, new_state, new_other_state) = match (
            mem::replace(this.state, StreamState::Nothing),
            mem::replace(this.other_state, StreamState::Nothing),
        ) {
            (StreamState::New(a), StreamState::New(b))
            | (StreamState::New(a), StreamState::Yielded(b))
            | (StreamState::Yielded(a), StreamState::New(b)) => (
                Poll::Ready(Some((this.combine)(&a, &b))),
                StreamState::Yielded(a),
                StreamState::Yielded(b),
            ),
            (StreamState::Nothing, _) if this.stream.is_done() => (
                Poll::Ready(None),
                StreamState::Nothing,
                StreamState::Nothing,
            ),
            (_, StreamState::Nothing) if this.other_stream.is_done() => (
                Poll::Ready(None),
                StreamState::Nothing,
                StreamState::Nothing,
            ),
            _ if this.stream.is_done() && this.other_stream.is_done() => (
                Poll::Ready(None),
                StreamState::Nothing,
                StreamState::Nothing,
            ),
            (a, b) => (Poll::Pending, a, b),
        };
        *this.state = new_state;
        *this.other_state = new_other_state;
        res
    }
}

impl<A, B, F, T> FusedStream for ZipLatestWith<A, B, F>
where
    A: Stream,
    B: Stream,
    F: FnMut(&A::Item, &B::Item) -> T,
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
    use futures::{executor::block_on, StreamExt};

    #[test]
    fn it_works() {
        let a = yield_on_none([Some(0), None, Some(1), None, None, Some(2)]);
        let b = yield_on_none([None, Some(10), Some(11), Some(12), None, None, Some(13)]);
        let expected = [10, 11, 13, 15];
        let actual = block_on(a.zip_latest_with(b, |i, j| i + j).collect::<Vec<_>>());
        assert_eq!(actual, expected);
    }
}
