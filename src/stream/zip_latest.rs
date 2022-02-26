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
    use crate::StreamTools;
    use futures::{channel::mpsc, executor::block_on, future::ready, StreamExt};
    use std::task::Poll;

    #[test]
    fn it_works() {
        let (mut waker_sender, waker_receiver) = mpsc::unbounded();
        let mut n = 0;
        let a = futures::stream::poll_fn({
            let mut waker_sender = waker_sender.clone();
            move |ctx| {
                let i = n;
                n += 1;
                match i {
                    0..=2 => Poll::Ready(Some(i)),
                    3 => {
                        waker_sender.unbounded_send(ctx.waker().clone()).unwrap();
                        Poll::Pending
                    }
                    4..=5 => Poll::Ready(Some(i)),
                    _ => {
                        waker_sender.disconnect();
                        Poll::Ready(None)
                    }
                }
            }
        });
        let mut n = 0;
        let b = futures::stream::poll_fn(move |ctx| {
            let i = n;
            n += 1;
            match i {
                0 => {
                    waker_sender.unbounded_send(ctx.waker().clone()).unwrap();
                    Poll::Pending
                }
                1..=5 => Poll::Ready(Some(i)),
                6 => {
                    waker_sender.unbounded_send(ctx.waker().clone()).unwrap();
                    Poll::Pending
                }
                7 => Poll::Ready(Some(i)),
                _ => {
                    waker_sender.disconnect();
                    Poll::Ready(None)
                }
            }
        });
        let drain_wakers = waker_receiver.for_each(|waker| ready(waker.wake()));
        let (c, _) = block_on(futures::future::join(
            a.zip_latest(b).collect::<Vec<_>>(),
            drain_wakers,
        ));
        assert_eq!(c, [(0, 1), (1, 2), (2, 3), (2, 4), (4, 5), (5, 5), (5, 7)])
    }
}
