use crate::{
    hypr::Hypr,
    path_builder::PathBuilder,
    server::signature::{Signature, Type},
};
use anyhow::{Error, Result};
use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap},
    env::VarError,
    fmt::{Debug, Write},
    path::Path,
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{unix::SocketAddr, UnixListener, UnixStream},
    sync::RwLock,
};
use tracing::{error, instrument};

mod signature;

#[derive(Debug, Default)]
pub struct Server {
    inner: RwLock<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    workspaces: HashMap<Arc<str>, WorkspaceSettings>,
    registers: BTreeMap<u8, Arc<str>>,
}

impl Server {
    pub async fn run(self: Arc<Self>) -> Result<()> {
        let instance = match std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
            Ok(instance) => instance,
            Err(VarError::NotPresent) => {
                error!("expected to be started in the context of a running hyprland instance");
                return Err(Error::msg(
                    "expected to be started in the context of a running hyprland instance",
                ));
            }
            Err(VarError::NotUnicode(var)) => {
                error!("invalid hyprland instance signature {var:?}, expected it to be unicode");
                return Err(Error::msg(
                    "invalid hyprland instance signature {var:?}, expected it to be unicode",
                ));
            }
        };

        let mut hypr_dir =
            PathBuilder::from_basepath(format_args!("/run/user/1000/hypr/{instance}"));

        let hypr_path: Arc<Path> = hypr_dir.with_filename(".socket.sock").into();
        let socket = UnixListener::bind(hypr_dir.with_filename("ws-mgr.sock"))?;

        while let Ok((stream, socket)) = socket.accept().await {
            tokio::spawn(Self::handle_message(
                Arc::clone(&self),
                stream,
                socket,
                Arc::clone(&hypr_path),
            ));
        }

        Ok(())
    }

    #[instrument]
    pub async fn handle_message(
        self: Arc<Self>,
        mut stream: UnixStream,
        socket: SocketAddr,
        path: Arc<Path>,
    ) -> Result<()> {
        let mut input = String::new();
        stream.read_to_string(&mut input).await?;

        let (cmd, input) =
            Signature::parse_cmd(&input).ok_or_else(|| Error::msg("expected param"))?;

        let mut hypr = Hypr::new();

        match cmd {
            "create" => {
                const CREATE: Signature = Signature {
                    cmd: "create",
                    params: &[("name", Type::Str)],
                };

                let mut parser = CREATE.parser(input);
                let name: &str = parser.parse_param()?;
                parser.finish()?;

                let mut lock = self.inner.write().await;
                match lock.workspaces.entry(name.into()) {
                    Entry::Vacant(vacant) => vacant.insert(WorkspaceSettings::default()),
                    Entry::Occupied(_) => return Err(Error::msg("name already in use")),
                };
            }
            "bind" => {
                const BIND: Signature = Signature {
                    cmd: "bind",
                    params: &[("name", Type::Str), ("register", Type::U8)],
                };

                let mut parser = BIND.parser(input);
                let name = parser.parse_param()?;
                let register = parser.parse_param()?;
                parser.finish()?;

                let mut lock = self.inner.write().await;
                let name = lock
                    .workspaces
                    .get_key_value(name)
                    .map(|(key, _)| key.clone())
                    .unwrap_or_else(|| Arc::from(name));

                lock.registers.insert(register, name);
            }
            "unbind" => {
                const UNBIND: Signature = Signature {
                    cmd: "unbind",
                    params: &[("register", Type::U8)],
                };

                let mut parser = UNBIND.parser(input);
                let register = parser.parse_param()?;
                parser.finish()?;

                let mut lock = self.inner.write().await;
                lock.registers.remove(&register);
            }
            "go_to" => {
                const GO_TO: Signature = Signature {
                    cmd: "go_to",
                    params: &[("register", Type::U8)],
                };

                let mut parser = GO_TO.parser(input);
                let register: u8 = parser.parse_param()?;
                parser.finish()?;

                hypr.go_to(register);
                hypr.send(&path).await?;
                hypr.clear()
            }
            "move_to" => {
                const MOVE_TO: Signature = Signature {
                    cmd: "move_to",
                    params: &[("register", Type::U8)],
                };

                let mut parser = MOVE_TO.parser(input);
                let register: u8 = parser.parse_param()?;
                parser.finish()?;

                hypr.move_to(register);
                hypr.send(&path).await?;
                hypr.clear()
            }
            "read" => {
                const READ: Signature = Signature {
                    cmd: "read",
                    params: &[("", Type::Opt), ("name", Type::Str)],
                };

                let mut parser = READ.parser(input);
                let name: Option<&str> = parser.parse_param()?;

                let lock = self.inner.read().await;
                if let Some(name) = name {
                    if let Some((name, settings)) = lock.workspaces.get_key_value(name) {
                        let mut buf = String::new();
                        writeln!(buf, "{name}: {settings:?}")
                            .expect("writing to string never fails");
                        stream.write_all(buf.as_bytes()).await?;
                    }
                } else {
                    let mut buf = String::new();
                    for (name, settings) in lock.workspaces.iter() {
                        writeln!(buf, "{name}: {settings:?}")
                            .expect("writing to string never fails");
                    }
                    stream.write_all(buf.as_bytes()).await?;
                }
            }
            _ => return Err(Error::msg("invalid command")),
        }

        stream.shutdown().await?;

        Ok(())
    }
}

#[derive(Debug)]
struct WorkspaceSettings {}

#[allow(clippy::derivable_impls)]
impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {}
    }
}
