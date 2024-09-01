use crate::{path_builder::PathBuilder, server::Server, socket::Socket};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{
    fmt::{Arguments, Write},
    sync::Arc,
};
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
    Bind { name: String, registry: u8 },
    Unbind { registry: u8 },
    Goto { registry: u8 },
    Moveto { registry: u8 },
    Read { name: Option<String> },
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.operation {
            Operation::Server => Arc::new(Server::default()).run().await,
            Operation::Create { name } => write_to_socket(format_args!("create {name}")).await,
            Operation::Bind { name, registry } => {
                write_to_socket(format_args!("bind {name} {registry}")).await
            }
            Operation::Unbind { registry } => {
                write_to_socket(format_args!("unbind {registry}")).await
            }
            Operation::Goto { registry } => write_to_socket(format_args!("goto {registry}")).await,
            Operation::Moveto { registry } => {
                write_to_socket(format_args!("moveto {registry}")).await
            }
            Operation::Read { name } => {
                if let Some(name) = name {
                    write_to_socket(format_args!("read {name}",)).await
                } else {
                    write_to_socket(format_args!("read",)).await
                }
            }
        }
    }
}

async fn write_to_socket(cmd: Arguments<'_>) -> Result<()> {
    let mut hypr_dir = PathBuilder::hypr_basepath()?;
    let mut socket = Socket::connect(hypr_dir.with_filename(Server::SOCKET)).await?;

    socket.write_fmt(cmd)?;
    socket.write_str("\nflush\n")?;
    socket.flush().await?;
    socket.inner.shutdown().await?;

    let out = socket.read_all().await?;
    let mut stdout = io::stdout();

    stdout.write_all(out).await?;
    stdout.flush().await?;

    Ok(())
}
