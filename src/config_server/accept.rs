use std::task::Poll;

use hyper::server::accept::{poll_fn, Accept};

use super::stream::StdStream;

pub fn accept_std_stream() -> impl Accept {
    let mut stream = Some(StdStream::default());
    poll_fn(move |_| Poll::Ready(stream.take().map(Ok::<_, ()>)))
}
