use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Sleep;

/// Wraps a transport so every read/write fails if no progress is made within
/// `timeout`. The timer resets on each successful read/write, so an actively
/// streaming transfer (a large body) never trips it; only a truly stalled
/// connection times out. This is what lets a hung IMAP session fail instead of
/// holding the `Syncing` state forever - the error then propagates to
/// `run_sync`, which moves the account to `Offline` so the next interval
/// retries automatically.
#[derive(Debug)]
pub(crate) struct TimeoutStream<T> {
    inner: T,
    timeout: Duration,
    read_timer: Option<Pin<Box<Sleep>>>,
    write_timer: Option<Pin<Box<Sleep>>>,
}

impl<T> TimeoutStream<T> {
    pub(crate) fn new(inner: T, timeout: Duration) -> Self {
        Self {
            inner,
            timeout,
            read_timer: None,
            write_timer: None,
        }
    }

    fn timed_out(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::TimedOut, message)
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for TimeoutStream<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.read_timer.is_none() {
            this.read_timer = Some(Box::pin(tokio::time::sleep(this.timeout)));
        }
        match Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Ready(result) => {
                this.read_timer = None;
                Poll::Ready(result)
            }
            Poll::Pending => match this.read_timer.as_mut().unwrap().as_mut().poll(cx) {
                Poll::Ready(()) => {
                    this.read_timer = None;
                    Poll::Ready(Err(Self::timed_out("imap read timed out")))
                }
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for TimeoutStream<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.write_timer.is_none() {
            this.write_timer = Some(Box::pin(tokio::time::sleep(this.timeout)));
        }
        match Pin::new(&mut this.inner).poll_write(cx, buf) {
            Poll::Ready(result) => {
                this.write_timer = None;
                Poll::Ready(result)
            }
            Poll::Pending => match this.write_timer.as_mut().unwrap().as_mut().poll(cx) {
                Poll::Ready(()) => {
                    this.write_timer = None;
                    Poll::Ready(Err(Self::timed_out("imap write timed out")))
                }
                Poll::Pending => Poll::Pending,
            },
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // A reader that never produces data and never registers a waker, simulating
    // a silently-stalled connection.
    struct StalledReader;

    impl AsyncRead for StalledReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Pending
        }
    }

    #[tokio::test]
    async fn read_times_out_when_transport_stalls() {
        let mut stream = TimeoutStream::new(StalledReader, Duration::from_millis(50));
        let mut buf = [0u8; 4];
        let result = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf))
            .await
            .expect("test itself should not time out");
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn write_times_out_when_transport_stalls() {
        struct StalledWriter;
        impl AsyncWrite for StalledWriter {
            fn poll_write(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
                _buf: &[u8],
            ) -> Poll<io::Result<usize>> {
                Poll::Pending
            }
            fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                Poll::Ready(Ok(()))
            }
            fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                Poll::Ready(Ok(()))
            }
        }
        let mut stream = TimeoutStream::new(StalledWriter, Duration::from_millis(50));
        let result = tokio::time::timeout(Duration::from_secs(2), stream.write(b"hi"))
            .await
            .expect("test itself should not time out");
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
}
