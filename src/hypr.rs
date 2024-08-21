use anyhow::Result;
use std::{
    fmt::{Display, Write},
    path::Path,
};
use tokio::{io::AsyncWriteExt, net::UnixStream};

#[derive(Debug)]
pub struct Hypr {
    buffer: String,
}

impl Hypr {
    pub fn new() -> Self {
        Self {
            buffer: String::from("[[BATCH]]"),
        }
    }

    pub fn clear(&mut self) {
        self.buffer.truncate("[[BATCH]]".len())
    }

    pub async fn send(&self, path: &Path) -> Result<()> {
        UnixStream::connect(path)
            .await?
            .write_all(self.buffer.as_bytes())
            .await?;
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

impl Default for Hypr {
    fn default() -> Self {
        Self::new()
    }
}
