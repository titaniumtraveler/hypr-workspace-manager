use anyhow::Result;
use std::{
    fmt::{self, Display, Formatter, Write},
    path::{Path, PathBuf},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};
use tracing::{debug, instrument};

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

    #[instrument(name = "hypr", skip(self, reply))]
    pub async fn send(&self, reply: Option<&mut String>) -> Result<()> {
        let mut socket = UnixStream::connect(&self.socket_path).await?;
        socket.write_all(self.buffer.as_bytes()).await?;
        debug!(request = &self.buffer, "request");
        socket.flush().await?;
        if let Some(reply) = reply {
            socket.read_to_string(reply).await?;
            debug!(reply = &reply, "reply");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Workspace<'a> {
    Id(i32),
    RelativeId(i32),
    RelativeMonitor(i32),
    RelativeMonitorEmpty(i32),
    RelativeOpen(i32),
    Previous,
    Empty,
    Name(&'a str),
    Special(Option<&'a str>),
}

impl Display for Workspace<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Workspace::Id(id) => write!(f, "{id}"),
            Workspace::RelativeId(rel) => write!(f, "{rel:+}"),
            Workspace::RelativeMonitor(rel) => write!(f, "m{rel:+}"),
            Workspace::RelativeMonitorEmpty(rel) => write!(f, "r{rel:+}"),
            Workspace::RelativeOpen(open) => write!(f, "e{open:+}"),
            Workspace::Previous => write!(f, "previous"),
            Workspace::Empty => write!(f, "empty"),
            Workspace::Name(name) => write!(f, "name:{name}"),
            Workspace::Special(None) => write!(f, "special"),
            Workspace::Special(Some(name)) => write!(f, "special:{name}"),
        }
    }
}

impl Hypr {
    pub fn go_to(&mut self, workspace: Workspace) {
        write!(self.buffer, "/dispatch workspace {workspace};")
            .expect("writing to string doesn't fail");
    }

    pub fn move_to(&mut self, workspace: Workspace) {
        write!(self.buffer, "/dispatch movetoworkspacesilent {workspace};")
            .expect("writing to string doesn't fail");
    }
}
