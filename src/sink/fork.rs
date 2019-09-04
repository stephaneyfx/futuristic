// Copyright (C) 2018-2019 Stephane Raux. Distributed under the MIT license.

use either::{Either, Left, Right};
use futures::{ready, Sink};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Sink returned by SinkTools::fork.
#[derive(Debug)]
pub struct Fork<T, LS, RS, F, LV, RV>
where
    LS: Sink<LV>,
    RS: Sink<RV>,
{
    left_sink: LS,
    right_sink: RS,
    state: ForkState<F, LV, RV>,
    phantom: PhantomData<fn(T)>,
}

#[derive(Debug)]
struct ForkState<F, LV, RV> {
    switch: F,
    left_closed: bool,
    right_closed: bool,
    buffer: Option<Either<LV, RV>>,
}

impl<T, LS, RS, F, LV, RV> Fork<T, LS, RS, F, LV, RV>
where
    F: FnMut(T) -> Either<LV, RV>,
    LS: Sink<LV>,
    RS: Sink<RV, Error = LS::Error>,
{
    pub(crate) fn new(left_sink: LS, right_sink: RS, switch: F) -> Self {
        let state = ForkState {
            switch,
            left_closed: false,
            right_closed: false,
            buffer: None,
        };
        Fork {
            left_sink,
            right_sink,
            state,
            phantom: PhantomData,
        }
    }

    fn state<'a>(self: Pin<&'a mut Self>) -> &'a mut ForkState<F, LV, RV> {
        unsafe { &mut self.get_unchecked_mut().state }
    }

    unsafe fn left_sink<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut LS> {
        self.map_unchecked_mut(|x| &mut x.left_sink)
    }

    unsafe fn right_sink<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut RS> {
        self.map_unchecked_mut(|x| &mut x.right_sink)
    }
}

impl<T, LS, RS, F, LV, RV> Sink<T> for Fork<T, LS, RS, F, LV, RV>
where
    F: FnMut(T) -> Either<LV, RV>,
    LS: Sink<LV>,
    RS: Sink<RV, Error = LS::Error>,
{
    type Error = LS::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        match self.state.buffer {
            Some(Left(_)) => unsafe { ready!(self.as_mut().left_sink().poll_ready(cx)?); }
            Some(Right(_)) => unsafe { ready!(self.as_mut().right_sink().poll_ready(cx)?); }
            None => return Poll::Ready(Ok(())),
        }
        let res = match self.as_mut().state().buffer.take() {
            Some(Left(item)) => unsafe { self.as_mut().left_sink().start_send(item) }
            Some(Right(item)) => unsafe { self.as_mut().right_sink().start_send(item) }
            None => unreachable!(),
        };
        Poll::Ready(res)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        assert!(self.state.buffer.is_none());
        let state = self.state();
        state.buffer = Some((state.switch)(item));
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_ready(cx)?);
        let left_res = unsafe { self.as_mut().left_sink().poll_flush(cx) };
        let right_res = unsafe { self.as_mut().right_sink().poll_flush(cx) };
        match (left_res?, right_res?) {
            (Poll::Ready(_), Poll::Ready(_)) => Poll::Ready(Ok(())),
            _ => Poll::Pending,
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_ready(cx)?);
        let left_res = if self.state.left_closed {
            Poll::Ready(Ok(()))
        } else {
            let res = unsafe { self.as_mut().left_sink().poll_close(cx) };
            if let Poll::Ready(Ok(_)) = res {
                self.as_mut().state().left_closed = true;
            }
            res
        };
        let right_res = if self.state.right_closed {
            Poll::Ready(Ok(()))
        } else {
            let res = unsafe { self.as_mut().right_sink().poll_close(cx) };
            if let Poll::Ready(Ok(_)) = res {
                self.as_mut().state().right_closed = true;
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
    use futures::{SinkExt, StreamExt};
    use futures::channel::mpsc;
    use futures::executor::block_on;
    use futures::stream;

    #[test]
    fn it_works() {
        let numbers = stream::iter(0 .. 10).map(Ok::<u32, ()>);
        let (even_sender, even_receiver) = mpsc::unbounded();
        let (odd_sender, odd_receiver) = mpsc::unbounded();
        let res = numbers.forward(
            even_sender
                .fork(odd_sender, |n| if n % 2 == 0 { Left(n) } else { Right(n) })
                .sink_map_err(|_| ())
        );
        block_on(res).unwrap();
        let (even_nums, odd_nums) = (0 .. 10).partition::<Vec<u32>, _>(|&n| n % 2 == 0);
        let received_evens = block_on(even_receiver.collect::<Vec<_>>());
        let received_odds = block_on(odd_receiver.collect::<Vec<_>>());
        assert_eq!(received_evens, even_nums);
        assert_eq!(received_odds, odd_nums);
    }
}
