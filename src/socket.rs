use crate::server::State;
use anyhow::Result;
use passfd::FdPassingExt;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Write},
    future::Future,
    io::{self, ErrorKind},
    os::fd::{AsRawFd, FromRawFd},
    path::Path,
    pin::{pin, Pin},
    str::from_utf8,
    task::{Context, Poll},
};
use tokio::io::Interest;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufStream},
    net::UnixStream,
};

pub struct Socket {
    pub inner: BufStream<UnixStream>,
    pub read_buf: Vec<u8>,
    pub write_buf: Vec<u8>,
}

impl Socket {
    pub fn from_unixstream(socket: UnixStream) -> Self {
        Self {
            inner: BufStream::new(socket),
            read_buf: Default::default(),
            write_buf: Default::default(),
        }
    }

    pub async fn connect(path: &Path) -> io::Result<Self> {
        let socket = UnixStream::connect(path).await?;
        Ok(Self::from_unixstream(socket))
    }

    pub async fn fetch_msg(&mut self) -> Result<bool> {
        self.read_buf.clear();
        self.inner.read_until(b'\n', &mut self.read_buf).await?;

        Ok(!self.read_buf.is_empty())
    }

    pub fn recv_fd_or_state(&mut self) -> RecvFdOrState<'_> {
        RecvFdOrState { socket: self }
    }

    pub fn msg(&self) -> Result<&str> {
        from_utf8(&self.read_buf).map_err(Into::into)
    }

    pub async fn read_all(&mut self) -> Result<&[u8]> {
        self.inner.read_to_end(&mut self.read_buf).await?;
        Ok(&self.read_buf)
    }

    pub async fn flush(&mut self) -> Result<()> {
        let res = self.inner.write_all(&self.write_buf).await;
        self.write_buf.clear();
        self.inner.flush().await?;
        res.map_err(Into::into)
    }
}

impl Socket {
    pub fn read_msg<'a, T: Deserialize<'a>>(&'a self) -> Result<T> {
        let mut de = serde_json::Deserializer::from_slice(&self.read_buf);
        let msg = Deserialize::deserialize(&mut de)?;
        de.end()?;

        Ok(msg)
    }

    pub fn write_msg<T: Serialize>(&mut self, msg: &T) -> Result<()> {
        let mut se = serde_json::Serializer::new(&mut self.write_buf);
        Serialize::serialize(msg, &mut se)?;
        self.write_buf.push(b'\n');
        Ok(())
    }
}

pub struct RecvFdOrState<'a> {
    socket: &'a mut Socket,
}

pub enum FdOrState {
    Socket(Socket),
    State((usize, State)),
}

impl Future for RecvFdOrState<'_> {
    type Output = Result<FdOrState>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Socket {
            inner: ref mut sock,
            read_buf: ref mut buf,
            ..
        } = self.socket;

        match (pin!(sock.read_until(b'\n', buf))).poll(cx)? {
            Poll::Ready(_) => {
                return Poll::Ready(Ok(FdOrState::State(self.socket.read_msg()?)));
            }
            Poll::Pending => {}
        }

        match pin!(recv_fd(sock.get_ref())).poll(cx)? {
            Poll::Ready(fd) => {
                return Poll::Ready(Ok(FdOrState::Socket(Socket::from_unixstream(fd))))
            }
            Poll::Pending => {}
        }

        Poll::Pending
    }
}

/// # Cancel Safety
///
/// As the future returned from [`UnixStream::readable()`] is cancel safe, so is this.
/// (The only future being `.await`ed)
async fn recv_fd(socket: &UnixStream) -> io::Result<UnixStream> {
    loop {
        socket.readable().await?;

        match socket.try_io(Interest::READABLE, || socket.as_raw_fd().recv_fd()) {
            Err(err) if err.kind() == ErrorKind::WouldBlock => continue,
            Ok(fd) => {
                // SAFETY: This assumes that only valid `UnixStream`s are sent over the
                // socket. This should be the case as long as nothing else than this server
                // is sending FDs over the socket.
                break unsafe {
                    UnixStream::from_std(std::os::unix::net::UnixStream::from_raw_fd(fd))
                };
            }

            Err(err) => break Err(err),
        }
    }
}

impl Write for Socket {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_buf.extend_from_slice(s.as_bytes());
        Ok(())
    }
}
