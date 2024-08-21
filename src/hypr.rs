use anyhow::Result;
use std::{
    fmt::{Display, Write},
    path::{Path, PathBuf},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

#[derive(Debug)]
pub struct Hypr {
    buffer: String,
    socket_path: PathBuf,
}

const BATCH: &str = "[[BATCH]]";

impl Hypr {
    pub fn new(socket_path: &Path) -> Self {
        Self {
            buffer: String::from(BATCH),
            socket_path: socket_path.into(),
        }
    }

    pub fn clear(&mut self) {
        self.buffer.truncate(BATCH.len())
    }

    /// Flush current buffer to socket and clear the buffer afterwards.
    ///
    /// Only actually sends, if the buffer contains messages to be sent.
    /// If an error occurs while sending, the buffer is not flushed!
    pub async fn flush(&mut self, reply: Option<&mut String>) -> Result<()> {
        if BATCH.len() < self.buffer.len() {
            self.send(reply).await?;
            self.clear();
        }
        Ok(())
    }

    pub async fn send(&self, reply: Option<&mut String>) -> Result<()> {
        let mut socket = UnixStream::connect(&self.socket_path).await?;
        socket.write_all(self.buffer.as_bytes()).await?;
        socket.flush().await?;
        if let Some(reply) = reply {
            socket.read_to_string(reply).await?;
        }
        Ok(())
    }
}

impl Hypr {
    pub fn go_to(&mut self, workspace: impl Display) {
        write!(self.buffer, "/dispatch workspace {workspace};")
            .expect("writing to string doesn't fail");
    }

    pub fn move_to(&mut self, workspace: impl Display) {
        write!(self.buffer, "/dispatch movetoworkspace {workspace};")
            .expect("writing to string doesn't fail");
    }
}
