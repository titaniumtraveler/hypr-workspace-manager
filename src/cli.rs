use crate::{
    server::{
        types::{Request, Workspace as WorkspaceRef},
        Server, State,
    },
    socket::Socket,
};
use anyhow::{anyhow, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Generator, Shell};
use std::{
    convert::Infallible,
    ffi::OsStr,
    fmt::Debug,
    io::{BufReader, BufWriter, Write},
    path::PathBuf,
    str::FromStr,
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
    Create {
        name: String,
    },
    Delete {
        name: String,
    },
    Bind {
        name: String,
        register: String,
    },
    Unbind {
        register: String,
    },
    GotoRegister {
        register: String,
    },
    MovetoRegister {
        register: String,
        #[arg(short, long)]
        focus: bool,
    },
    GotoName {
        name: String,
    },
    MovetoName {
        name: String,
        #[arg(short, long)]
        focus: bool,
    },
    Read {
        workspace: Option<Workspace>,
    },
    Write {},
    Completions {
        shell: Shell,
        #[arg(long)]
        save: bool,
    },
}

#[derive(Debug, Clone)]
enum Workspace {
    Workspace(String),
    Register(String),
}

impl Workspace {
    fn as_workspace_ref(&self) -> WorkspaceRef<'_> {
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
            Operation::MovetoRegister { register, focus } => {
                write_to_socket(Request::MovetoRegister {
                    register: &register,
                    focus,
                })
                .await
            }
            Operation::GotoName { ref name } => write_to_socket(Request::GotoName { name }).await,
            Operation::MovetoName { ref name, focus } => {
                write_to_socket(Request::MovetoName { name, focus }).await
            }
            Operation::Read { workspace } => {
                write_to_socket(Request::Read {
                    workspace: workspace.as_ref().map(Workspace::as_workspace_ref),
                })
                .await
            }
            Operation::Write {} => {
                let State {
                    workspaces,
                    registers,
                } = serde_json::from_reader(BufReader::new(std::io::stdin()))?;
                write_to_socket(Request::Write(State {
                    workspaces,
                    registers,
                }))
                .await
            }
            Operation::Completions { shell, save } => {
                const BIN_NAME: &str = match option_env!("CARGO_BIN_NAME") {
                    Some(name) => name,
                    None => env!("CARGO_PKG_NAME"),
                };

                fn gen<G: Generator, W: Write>(shell: G, writer: W) {
                    clap_complete::generate(
                        shell,
                        &mut Cli::command(),
                        BIN_NAME,
                        &mut BufWriter::new(writer),
                    );
                }

                if save {
                    let path = match shell {
                        Shell::Bash => {
                            let mut path = PathBuf::new();

                            if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
                                path.extend([&data_home]);
                            } else if let Some(home) = std::env::var_os("HOME") {
                                path.extend([&home, OsStr::new(".local/share")]);
                            }
                            path.extend(["bash-completion/completions/", BIN_NAME]);

                            path
                        }
                        shell => {
                            return Err(anyhow!(
                                "saving completions to file isn't yet supported by {shell}"
                            ))
                        }
                    };

                    println!(
                        "writing completions for `{shell}` to `{path}`",
                        path = path.display()
                    );
                    gen(shell, std::fs::File::create(path)?);
                } else {
                    gen(shell, std::io::stdout());
                }

                Ok(())
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
