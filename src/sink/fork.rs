// Copyright (C) 2018 Stephane Raux. Distributed under the MIT license.

use either::{Either, Left, Right};
use futures::{Async, AsyncSink, Poll, Sink, StartSend, try_ready};
use std::marker::PhantomData;

/// Sink returned by SinkTools::fork.
#[derive(Debug)]
pub struct Fork<T, LS, RS, F>
where
    LS: Sink,
    RS: Sink,
{
    switch: F,
    left_sink: LS,
    right_sink: RS,
    buffer: Option<Either<LS::SinkItem, RS::SinkItem>>,
    phantom: PhantomData<fn(T)>,
}

impl<T, LS, RS, F> Fork<T, LS, RS, F>
where
    F: FnMut(T) -> Either<LS::SinkItem, RS::SinkItem>,
    LS: Sink,
    RS: Sink<SinkError = LS::SinkError>,
{
    pub (crate) fn new(left_sink: LS, right_sink: RS, switch: F) -> Self {
        Fork {
            switch,
            left_sink: left_sink,
            right_sink: right_sink,
            buffer: None,
            phantom: PhantomData,
        }
    }

    fn poll(&mut self) -> Poll<(), LS::SinkError> {
        match self.buffer.take() {
            Some(Left(item)) => {
                if let AsyncSink::NotReady(item) =
                    self.left_sink.start_send(item)?
                {
                    self.buffer = Some(Left(item));
                }
            }
            Some(Right(item)) => {
                if let AsyncSink::NotReady(item) =
                    self.right_sink.start_send(item)?
                {
                    self.buffer = Some(Right(item));
                }
            }
            None => {}
        }
        if self.buffer.is_none() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }
}

impl<T, LS, RS, F> Sink for Fork<T, LS, RS, F>
where
    F: FnMut(T) -> Either<LS::SinkItem, RS::SinkItem>,
    LS: Sink,
    RS: Sink<SinkError = LS::SinkError>,
{
    type SinkItem = T;
    type SinkError = LS::SinkError;

    fn start_send(&mut self, item: T) -> StartSend<T, Self::SinkError> {
        if self.poll()?.is_not_ready() {return Ok(AsyncSink::NotReady(item))}
        self.buffer = Some((self.switch)(item));
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        let ready = self.poll()?;
        try_ready!(self.left_sink.poll_complete());
        try_ready!(self.right_sink.poll_complete());
        Ok(ready)
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        try_ready!(self.poll());
        self.left_sink.close()?;
        self.right_sink.close()?;
        Ok(Async::Ready(()))
    }
}

#[cfg(test)]
mod tests {
    use crate::SinkTools;
    use either::{Left, Right};
    use futures::{Future, Sink, Stream};
    use futures::stream;
    use futures::sync::mpsc;

    #[test]
    fn it_works() {
        let numbers = stream::iter_ok::<_, ()>(0..10);
        let (even_sender, even_receiver) = mpsc::unbounded();
        let (odd_sender, odd_receiver) = mpsc::unbounded();
        numbers.forward(
            even_sender.fork(
                odd_sender,
                |n| if n % 2 == 0 {Left(n)} else {Right(n)}
            )
                .sink_map_err(|_| ())
        ).wait().map(|_| ()).unwrap();
        let (even_nums, odd_nums) = (0..10).partition::<Vec<i32>, _>(
            |&n| n % 2 == 0);
        assert_eq!(even_receiver.collect().wait(), Ok(even_nums));
        assert_eq!(odd_receiver.collect().wait(), Ok(odd_nums));
    }
}
