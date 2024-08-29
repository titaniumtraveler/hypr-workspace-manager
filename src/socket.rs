use anyhow::{Ok, Result};
use std::{
    fmt::{self, Write},
    path::Path,
    str::from_utf8,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufStream},
    net::UnixStream,
};

pub struct Socket {
    pub inner: BufStream<UnixStream>,
    pub read_buf: Vec<u8>,
    pub write_buf: String,
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

    pub async fn read_msg<'msg>(
        stream: &mut BufStream<UnixStream>,
        read_buf: &'msg mut Vec<u8>,
    ) -> Result<Option<&'msg str>> {
        read_buf.clear();
        stream.read_until(b'\n', read_buf).await?;

        if read_buf.is_empty() {
            return Ok(None);
        }

        let message = from_utf8(read_buf)?.trim_end_matches('\n');
        Ok(Some(message))
    }

    pub async fn flush(&mut self) -> Result<()> {
        let res = self.inner.write_all(self.write_buf.as_bytes()).await;
        self.write_buf.clear();
        res.map_err(Into::into)
    }
}

impl Write for Socket {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_buf.write_str(s)
    }
}
