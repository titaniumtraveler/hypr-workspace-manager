use crate::{
    server::{
        types::{Request, Workspace as WorkspaceRef},
        Server, State,
    },
    socket::Socket,
};
use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use std::{convert::Infallible, fmt::Debug, str::FromStr, sync::Arc};
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
    Delete { name: String },
    Bind { name: String, register: String },
    Unbind { register: String },
    GotoRegister { register: String },
    MovetoRegister { register: String },
    GotoName { name: String },
    MovetoName { name: String },
    Read { workspace: Option<Workspace> },
    Write { state: String },
    Completions { shell: Shell },
}

#[derive(Debug, Clone)]
enum Workspace {
    Workspace(String),
    Register(String),
}

impl Workspace {
    fn as_workspace_ref(&self) -> WorkspaceRef {
        match self {
            Workspace::Workspace(name) => WorkspaceRef::Workspace(name),
            Workspace::Register(register) => WorkspaceRef::Register(register),
        }
    }
}

impl FromStr for Workspace {
    type Err = Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(s.parse()
            .map(Workspace::Register)
            .unwrap_or_else(|_| Workspace::Workspace(s.to_owned())))
    }
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.operation {
            Operation::Server => Arc::new(Server::default()).run().await,
            Operation::Create { name } => write_to_socket(Request::Create { name: &name }).await,
            Operation::Delete { name } => write_to_socket(Request::Delete { name: &name }).await,
            Operation::Bind { name, register } => {
                write_to_socket(Request::Bind {
                    name: &name,
                    register: &register,
                })
                .await
            }
            Operation::Unbind { register } => {
                write_to_socket(Request::Unbind {
                    register: &register,
                })
                .await
            }
            Operation::GotoRegister { register } => {
                write_to_socket(Request::GotoRegister {
                    register: &register,
                })
                .await
            }
            Operation::MovetoRegister { register } => {
                write_to_socket(Request::MovetoRegister {
                    register: &register,
                })
                .await
            }
            Operation::GotoName { ref name } => write_to_socket(Request::GotoName { name }).await,
            Operation::MovetoName { ref name } => {
                write_to_socket(Request::MovetoName { name }).await
            }
            Operation::Read { workspace } => {
                write_to_socket(Request::Read {
                    workspace: workspace.as_ref().map(Workspace::as_workspace_ref),
                })
                .await
            }
            Operation::Completions { shell } => {
                clap_complete::generate(
                    shell,
                    &mut Cli::command(),
                    option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
                    &mut std::io::stdout(),
                );
                Ok(())
            }
            Operation::Write { state } => {
                let State {
                    workspaces,
                    registers,
                } = serde_json::from_str(&state)?;
                write_to_socket(Request::Write(State {
                    workspaces,
                    registers,
                }))
                .await
            }
        }
    }
}

async fn write_to_socket(request: Request<'_>) -> Result<()> {
    let path = crate::socket::socket_path()?;
    let mut socket = Socket::connect(&path).await?;

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
