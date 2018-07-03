// Copyright (C) 2018 Stephane Raux. Distributed under the MIT license.

use futures::{Async, AsyncSink, Poll, Sink, StartSend};
use std::marker::PhantomData;

/// Sink that accepts and ignores everything sent to it.
///
/// `T` and `E` are respectively the sink item and error types.
#[derive(Debug)]
pub struct Null<T, E> {
    item_phantom: PhantomData<fn(T)>,
    error_phantom: PhantomData<fn(E)>,
}

impl<T, E> Null<T, E> {
    /// Creates a new `Null` sink.
    pub fn new() -> Self {
        Null {item_phantom: PhantomData, error_phantom: PhantomData}
    }
}

impl<T, E> Default for Null<T, E> {
    fn default() -> Self {
        Null::new()
    }
}

impl<T, E> Sink for Null<T, E> {
    type SinkItem = T;
    type SinkError = E;

    fn start_send(&mut self, _: T) -> StartSend<T, E> {
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), E> {
        Ok(Async::Ready(()))
    }
}

#[cfg(test)]
mod tests {
    use futures::{Future, Stream};
    use futures::stream;
    use super::Null;

    #[test]
    fn it_works() {
        stream::iter_ok::<_, ()>(0..10).forward(Null::new()).wait()
            .map(|_| ())
            .unwrap();
    }
}
