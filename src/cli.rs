use crate::{
    path_builder::PathBuilder,
    server::{types::Request, Server},
    socket::Socket,
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tokio::io::{self, AsyncWriteExt};

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(subcommand)]
    operation: Operation,
}

#[derive(Debug, Subcommand)]
enum Operation {
    Server,
    Create { name: String },
    Bind { name: String, register: u8 },
    Unbind { register: u8 },
    Goto { register: u8 },
    Moveto { register: u8 },
    Read { name: Option<String> },
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.operation {
            Operation::Server => Arc::new(Server::default()).run().await,
            Operation::Create { name } => write_to_socket(Request::Create { name: &name }).await,
            Operation::Bind { name, register } => {
                write_to_socket(Request::Bind {
                    name: &name,
                    register,
                })
                .await
            }
            Operation::Unbind { register } => write_to_socket(Request::Unbind { register }).await,
            Operation::Goto { register } => write_to_socket(Request::Goto { register }).await,
            Operation::Moveto { register } => write_to_socket(Request::Moveto { register }).await,
            Operation::Read { name } => {
                write_to_socket(Request::Read {
                    name: name.as_deref(),
                })
                .await
            }
        }
    }
}

async fn write_to_socket(request: Request<'_>) -> Result<()> {
    let mut hypr_dir = PathBuilder::hypr_basepath()?;
    let mut socket = Socket::connect(hypr_dir.with_filename(Server::SOCKET)).await?;

    socket.write_msg(&request)?;
    socket.write_msg(&Request::Flush)?;
    socket.flush().await?;
    socket.inner.shutdown().await?;

    let out = socket.read_all().await?;
    let mut stdout = io::stdout();

    stdout.write_all(out).await?;
    stdout.flush().await?;

    Ok(())
}
