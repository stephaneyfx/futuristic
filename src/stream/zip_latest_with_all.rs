// Copyright (C) 2022 Stephane Raux. Distributed under the 0BSD license.

use futures::{
    future::{join_all, JoinAll},
    stream::{FusedStream, FuturesUnordered, StreamFuture},
    Stream, StreamExt,
};
use pin_project::pin_project;
use std::{
    fmt::{self, Debug},
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

/// Stream returned by [`zip_latest_with_all`](crate::stream::zip_latest_with_all).
#[pin_project]
pub struct ZipLatestWithAll<S, F>
where
    S: Stream + Unpin,
{
    inner: Inner<S>,
    combine: F,
}

impl<S, F, T> ZipLatestWithAll<S, F>
where
    S: Stream + Unpin,
    F: FnMut(&[S::Item]) -> T,
{
    pub(crate) fn new<I>(streams: I, combine: F) -> Self
    where
        I: IntoIterator<Item = S>,
    {
        Self {
            inner: Inner::Fill(join_all(streams.into_iter().map(|s| s.into_future()))),
            combine,
        }
    }
}

impl<S, F> Debug for ZipLatestWithAll<S, F>
where
    S: Stream + Unpin,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ZipLatestWithAll")
    }
}

impl<S, F, T> Stream for ZipLatestWithAll<S, F>
where
    S: Stream + Unpin,
    F: FnMut(&[S::Item]) -> T,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        match this.inner {
            Inner::Fill(all) => {
                let items_and_streams = ready!(Pin::new(all).poll(ctx));
                let (res, inner) = items_and_streams
                    .into_iter()
                    .try_fold(
                        (Vec::new(), FuturesUnordered::new()),
                        |(mut items, next_items), (item, stream)| {
                            let i = items.len();
                            items.push(item?);
                            next_items.push(IndexedStream::new(i, stream).into_future());
                            Some((items, next_items))
                        },
                    )
                    .map(|(items, next_items)| {
                        (
                            Some((this.combine)(&items)),
                            Inner::Filled(Filled { items, next_items }),
                        )
                    })
                    .unwrap_or_else(|| (None, Inner::Filled(Default::default())));
                *this.inner = inner;
                Poll::Ready(res)
            }
            Inner::Filled(Filled { items, next_items }) => {
                let mut yielded = Vec::new();
                loop {
                    match Pin::new(&mut *next_items).poll_next(ctx) {
                        Poll::Ready(Some((Some((i, head)), tail))) => {
                            items[i] = head;
                            yielded.push(tail);
                        }
                        Poll::Ready(Some((None, _))) => {}
                        Poll::Ready(None) => {
                            let res = Some(&*items)
                                .filter(|_| !yielded.is_empty())
                                .map(|items| (this.combine)(items));
                            next_items.extend(yielded.into_iter().map(|s| s.into_future()));
                            break Poll::Ready(res);
                        }
                        Poll::Pending => {
                            let res = Some(&*items)
                                .filter(|_| !yielded.is_empty())
                                .map(|items| (this.combine)(items));
                            next_items.extend(yielded.into_iter().map(|s| s.into_future()));
                            break res.map_or(Poll::Pending, |items| Poll::Ready(Some(items)));
                        }
                    }
                }
            }
        }
    }
}

impl<S, F, T> FusedStream for ZipLatestWithAll<S, F>
where
    S: Stream + Unpin,
    F: FnMut(&[S::Item]) -> T,
{
    fn is_terminated(&self) -> bool {
        match &self.inner {
            Inner::Filled(Filled { next_items, .. }) => next_items.is_terminated(),
            _ => false,
        }
    }
}

enum Inner<S: Stream + Unpin> {
    Fill(JoinAll<StreamFuture<S>>),
    Filled(Filled<S>),
}

impl<S: Stream + Unpin> Unpin for Inner<S> {}

struct Filled<S: Stream + Unpin> {
    items: Vec<S::Item>,
    next_items: FuturesUnordered<StreamFuture<IndexedStream<S>>>,
}

impl<S: Stream + Unpin> Default for Filled<S> {
    fn default() -> Self {
        Filled {
            items: Vec::new(),
            next_items: Default::default(),
        }
    }
}

struct IndexedStream<S> {
    i: usize,
    s: S,
}

impl<S> IndexedStream<S> {
    fn new(i: usize, s: S) -> Self {
        Self { i, s }
    }
}

impl<S: Stream + Unpin> Stream for IndexedStream<S> {
    type Item = (usize, S::Item);

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let x = ready!(Pin::new(&mut self.s).poll_next(ctx));
        Poll::Ready(x.map(|x| (self.i, x)))
    }
}

#[cfg(test)]
mod tests {
    use crate::stream::{test_util::yield_on_none, zip_latest_with_all};
    use futures::{executor::block_on, pin_mut, StreamExt};

    #[test]
    fn it_works() {
        let a = yield_on_none([Some(0), None, Some(1), None, None, Some(2)]);
        pin_mut!(a);
        let b = yield_on_none([None, Some(10), Some(11), Some(12), None, None, Some(13)]);
        pin_mut!(b);
        let expected = [10, 12, 13, 14, 15];
        let actual = block_on(
            zip_latest_with_all([a.left_stream(), b.right_stream()], |items| {
                items.iter().sum::<i32>()
            })
            .collect::<Vec<_>>(),
        );
        assert_eq!(actual, expected);
    }
}
