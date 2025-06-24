use anyhow::Context;
use niri_ipc::{Request, Response};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

pub struct Niri {
    socket: BufReader<UnixStream>,
    buffer: Vec<u8>,
}

impl Niri {
    pub async fn from_env() -> anyhow::Result<Self> {
        let socket = UnixStream::connect(
            std::env::var_os(niri_ipc::socket::SOCKET_PATH_ENV)
                .context("failed to get niri socket")?,
        )
        .await?;

        Ok(Self {
            socket: BufReader::new(socket),
            buffer: Vec::new(),
        })
    }

    pub async fn request(&mut self, req: &Request) -> anyhow::Result<Response> {
        self.buffer.clear();
        serde_json::to_writer(&mut self.buffer, req)?;
        self.buffer.push(b'\n');
        self.socket.write_all(&self.buffer).await?;

        self.buffer.clear();
        self.socket.read_until(b'\n', &mut self.buffer).await?;

        serde_json::from_slice(&self.buffer).map_err(Into::into)
    }
}
