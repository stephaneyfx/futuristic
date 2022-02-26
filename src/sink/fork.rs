// Copyright (C) 2018-2022 Stephane Raux. Distributed under the 0BSD license.

use either::{Either, Left, Right};
use futures::{ready, Sink};
use pin_project::pin_project;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Sink returned by [`SinkTools::fork`](crate::SinkTools::fork).
#[pin_project]
#[derive(Debug)]
pub struct Fork<T, LS, RS, F, LV, RV>
where
    LS: Sink<LV>,
    RS: Sink<RV>,
{
    #[pin]
    left_sink: LS,
    #[pin]
    right_sink: RS,
    switch: F,
    left_closed: bool,
    right_closed: bool,
    buffer: Option<Either<LV, RV>>,
    phantom: PhantomData<fn(T)>,
}

impl<T, LS, RS, F, LV, RV> Fork<T, LS, RS, F, LV, RV>
where
    F: FnMut(T) -> Either<LV, RV>,
    LS: Sink<LV>,
    RS: Sink<RV, Error = LS::Error>,
{
    pub(crate) fn new(left_sink: LS, right_sink: RS, switch: F) -> Self {
        Fork {
            left_sink,
            right_sink,
            switch,
            left_closed: false,
            right_closed: false,
            buffer: None,
            phantom: PhantomData,
        }
    }
}

impl<T, LS, RS, F, LV, RV> Sink<T> for Fork<T, LS, RS, F, LV, RV>
where
    F: FnMut(T) -> Either<LV, RV>,
    LS: Sink<LV>,
    RS: Sink<RV, Error = LS::Error>,
{
    type Error = LS::Error;

    fn poll_ready(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        let (res, buffer) = match this.buffer.take() {
            Some(Left(item)) => match this.left_sink.as_mut().poll_ready(ctx) {
                Poll::Ready(Ok(())) => (Poll::Ready(this.left_sink.start_send(item)), None),
                res => (res, Some(Left(item))),
            },
            Some(Right(item)) => match this.right_sink.as_mut().poll_ready(ctx) {
                Poll::Ready(Ok(())) => (Poll::Ready(this.right_sink.start_send(item)), None),
                res => (res, Some(Right(item))),
            },
            None => (Poll::Ready(Ok(())), None),
        };
        *this.buffer = buffer;
        res
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        let this = self.project();
        assert!(this.buffer.is_none());
        *this.buffer = Some((this.switch)(item));
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_ready(ctx)?);
        let this = self.project();
        let left_res = this.left_sink.poll_flush(ctx);
        let right_res = this.right_sink.poll_flush(ctx);
        match (left_res?, right_res?) {
            (Poll::Ready(_), Poll::Ready(_)) => Poll::Ready(Ok(())),
            _ => Poll::Pending,
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_ready(ctx)?);
        let this = self.project();
        let left_res = if *this.left_closed {
            Poll::Ready(Ok(()))
        } else {
            let res = this.left_sink.poll_close(ctx);
            if let Poll::Ready(Ok(_)) = res {
                *this.left_closed = true;
            }
            res
        };
        let right_res = if *this.right_closed {
            Poll::Ready(Ok(()))
        } else {
            let res = this.right_sink.poll_close(ctx);
            if let Poll::Ready(Ok(_)) = res {
                *this.right_closed = true;
            }
            res
        };
        match (left_res?, right_res?) {
            (Poll::Ready(_), Poll::Ready(_)) => Poll::Ready(Ok(())),
            _ => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::SinkTools;
    use either::{Left, Right};
    use futures::channel::mpsc;
    use futures::executor::block_on;
    use futures::stream;
    use futures::{SinkExt, StreamExt};

    #[test]
    fn it_works() {
        let numbers = stream::iter(0..10).map(Ok::<u32, ()>);
        let (even_sender, even_receiver) = mpsc::unbounded();
        let (odd_sender, odd_receiver) = mpsc::unbounded();
        let res = numbers.forward(
            even_sender
                .fork(odd_sender, |n| if n % 2 == 0 { Left(n) } else { Right(n) })
                .sink_map_err(|_| ()),
        );
        block_on(res).unwrap();
        let (even_nums, odd_nums) = (0..10).partition::<Vec<u32>, _>(|&n| n % 2 == 0);
        let received_evens = block_on(even_receiver.collect::<Vec<_>>());
        let received_odds = block_on(odd_receiver.collect::<Vec<_>>());
        assert_eq!(received_evens, even_nums);
        assert_eq!(received_odds, odd_nums);
    }
}
