// Copyright (C) 2022 Stephane Raux. Distributed under the 0BSD license.

use futures::{
    future::{join_all, JoinAll},
    stream::{FusedStream, FuturesUnordered, StreamFuture},
    Stream, StreamExt,
};
use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

/// Stream returned by [`zip_latest_all`](crate::stream::zip_latest_all).
pub struct ZipLatestAll<S: Stream + Unpin>(Inner<S>);

impl<S> ZipLatestAll<S>
where
    S: Stream + Unpin,
{
    pub(crate) fn new<I>(streams: I) -> Self
    where
        I: IntoIterator<Item = S>,
    {
        Self(Inner::Fill(join_all(
            streams.into_iter().map(|s| s.into_future()),
        )))
    }
}

impl<S> Stream for ZipLatestAll<S>
where
    S: Stream + Unpin,
    S::Item: Clone,
{
    type Item = Vec<S::Item>;

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut self.0 {
            Inner::Fill(all) => {
                let items_and_streams = ready!(Pin::new(all).poll(ctx));
                let (items, inner) = items_and_streams
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
                            Some(items.clone()),
                            Inner::Filled(Filled { items, next_items }),
                        )
                    })
                    .unwrap_or_else(|| (None, Inner::Filled(Default::default())));
                self.0 = inner;
                Poll::Ready(items)
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
                            let items = Some(&*items).filter(|_| !yielded.is_empty()).cloned();
                            next_items.extend(yielded.into_iter().map(|s| s.into_future()));
                            break Poll::Ready(items);
                        }
                        Poll::Pending => {
                            let items = Some(&*items).filter(|_| !yielded.is_empty()).cloned();
                            next_items.extend(yielded.into_iter().map(|s| s.into_future()));
                            break items.map_or(Poll::Pending, |items| Poll::Ready(Some(items)));
                        }
                    }
                }
            }
        }
    }
}

impl<S> FusedStream for ZipLatestAll<S>
where
    S: Stream + Unpin,
    S::Item: Clone,
{
    fn is_terminated(&self) -> bool {
        match &self.0 {
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
    use crate::stream::{test_util::yield_on_none, zip_latest_all};
    use futures::{
        executor::block_on,
        pin_mut,
        stream::{empty, repeat},
        StreamExt,
    };

    #[test]
    fn it_works() {
        let a = yield_on_none([Some(0), None, Some(1), None, None, Some(2)]);
        pin_mut!(a);
        let b = yield_on_none([None, Some(10), Some(11), Some(12), None, None, Some(13)]);
        pin_mut!(b);
        let expected = [
            vec![0, 10],
            vec![1, 11],
            vec![1, 12],
            vec![2, 12],
            vec![2, 13],
        ];
        let actual =
            block_on(zip_latest_all([a.left_stream(), b.right_stream()]).collect::<Vec<_>>());
        assert_eq!(actual, expected);
    }

    #[test]
    fn zipping_latest_of_2_empty_streams_gives_empty_stream() {
        let r = block_on(zip_latest_all([empty::<()>(), empty()]).collect::<Vec<_>>());
        assert_eq!(r, <[Vec<()>; 0]>::default());
    }

    #[test]
    fn zipping_latest_of_empty_and_infinite_streams_gives_empty_stream() {
        let r = block_on(
            zip_latest_all([empty::<()>().left_stream(), repeat(()).right_stream()])
                .collect::<Vec<_>>(),
        );
        assert_eq!(r, <[Vec<()>; 0]>::default());
    }
}
