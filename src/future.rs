// Copyright (C) 2022 Stephane Raux. Distributed under the 0BSD license.

//! Tools for futures

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// Returns a `Future` that returns `Pending` the first time it is polled and `Ready` afterwards.
pub fn yield_now() -> YieldNow {
    YieldNow(false)
}

/// Future returned by [`yield_now`]
#[derive(Debug)]
pub struct YieldNow(bool);

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            ctx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::future::yield_now;
    use futures::{executor::block_on, FutureExt};
    use std::future::ready;

    #[test]
    fn it_works() {
        assert_eq!(
            block_on(futures::future::select(ready(1), yield_now().map(|_| 2)))
                .factor_first()
                .0,
            1,
        );
        assert_eq!(
            block_on(futures::future::select(yield_now().map(|_| 2), ready(1)))
                .factor_first()
                .0,
            1,
        );
    }
}
