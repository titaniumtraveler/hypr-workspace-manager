use anyhow::Result;
use std::{
    fmt::{self, Write},
    path::Path,
    str::from_utf8,
};
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

    pub async fn connect(path: &Path) -> Result<Self> {
        let socket = UnixStream::connect(path).await?;
        Ok(Self::from_unixstream(socket))
    }

    pub async fn fetch_msg(&mut self) -> Result<bool> {
        self.read_buf.clear();
        self.inner.read_until(b'\n', &mut self.read_buf).await?;

        Ok(!self.read_buf.is_empty())
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

impl Write for Socket {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_buf.extend_from_slice(s.as_bytes());
        Ok(())
    }
}
