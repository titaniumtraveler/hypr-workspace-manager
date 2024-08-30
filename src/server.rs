use crate::{
    hypr::{Hypr, Workspace},
    path_builder::PathBuilder,
    server::signature::{Signature, Type},
    socket::Socket,
};
use anyhow::{anyhow, Result};
use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap},
    fmt::Write,
    io::ErrorKind,
    path::Path,
    sync::Arc,
};
use tokio::{
    fs::remove_file,
    net::{unix::SocketAddr, UnixListener},
    sync::RwLock,
};
use tracing::{debug, error, info, info_span, instrument, warn, Instrument};

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
    #[instrument(name = "socket server", skip(self), err)]
    pub async fn run(self: Arc<Self>) -> Result<()> {
        let mut hypr_dir = PathBuilder::hypr_basepath()?;

        let hypr_path: Arc<Path> = hypr_dir.with_filename(".socket.sock").into();
        let socket = hypr_dir.with_filename("ws-mgr.sock");
        if let Err(err) = remove_file(socket).await {
            if err.kind() != ErrorKind::NotFound {
                return Err(err.into());
            }
        }
        let socket = UnixListener::bind(socket)?;

        while let Ok((stream, socket)) = socket.accept().await {
            tokio::spawn({
                let server_state = Arc::clone(&self);
                let hypr_path = Arc::clone(&hypr_path);

                async {
                    let res = server_state
                        .handle_client(Socket::from_unixstream(stream), socket, hypr_path)
                        .await;
                    if let Err(err) = res {
                        error!(?err, "client failed with {err}");
                    }
                }
                .instrument(info_span!("client"))
            });
        }

        Ok(())
    }

    pub async fn handle_client(
        self: Arc<Self>,
        mut stream: Socket,
        _: SocketAddr,
        hypr_path: Arc<Path>,
    ) -> Result<()> {
        info!("connected");

        let mut hypr = Hypr::new(&hypr_path);

        loop {
            let res = async {
                debug!("waiting for input");
                if !stream.fetch_msg().await? {
                    return Ok(false);
                }

                if let Err(err) = self.handle_message(&mut stream, &mut hypr).await {
                    warn!(?err, "error processing message");

                    write!(stream, "{}", err)?;
                    stream.flush().await?;
                }

                Result::<_, anyhow::Error>::Ok(true)
            }
            .instrument(info_span!("message"))
            .await;

            if !res? {
                break;
            }
        }

        hypr.flush(Some(&mut stream.write_buf)).await?;
        stream.flush().await?;

        info!("disconnected");

        Ok(())
    }

    pub async fn handle_message<'a>(&self, stream: &'a mut Socket, hypr: &mut Hypr) -> Result<()> {
        let msg = stream.msg()?;
        debug!(msg, "input");
        let (cmd, input) = Signature::parse_cmd(msg).ok_or_else(|| anyhow!("expected param"))?;
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
                    Entry::Occupied(_) => return Err(anyhow!("name already in use")),
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
            "goto" => {
                const GO_TO: Signature = Signature {
                    cmd: "goto",
                    params: &[("register", Type::U8)],
                };

                let mut parser = GO_TO.parser(input);
                let register: u8 = parser.parse_param()?;
                parser.finish()?;

                let lock = self.inner.read().await;
                let name = lock.registers.get(&register).ok_or_else(|| {
                    anyhow!("register {register} does not point to any workspace")
                })?;

                hypr.go_to(Workspace::Name(name));
            }
            "moveto" => {
                const MOVE_TO: Signature = Signature {
                    cmd: "moveto",
                    params: &[("register", Type::U8)],
                };

                let mut parser = MOVE_TO.parser(input);
                let register: u8 = parser.parse_param()?;
                parser.finish()?;

                let lock = self.inner.read().await;
                let name = lock.registers.get(&register).ok_or_else(|| {
                    anyhow!("register {register} does not point to any workspace")
                })?;

                hypr.move_to(Workspace::Name(name));
            }
            "read" => {
                const READ: Signature = Signature {
                    cmd: "read",
                    params: &[("", Type::Opt), ("name", Type::Str)],
                };

                let mut parser = READ.parser(input);
                let name: Option<&str> = parser.parse_param()?;
                parser.finish()?;

                let lock = self.inner.read().await;
                if let Some(name) = name {
                    if let Some((name, settings)) = lock.workspaces.get_key_value(name) {
                        writeln!(stream, "{name}: {settings:?}")
                            .expect("writing to string never fails");
                    }
                } else {
                    for (name, settings) in lock.workspaces.iter() {
                        writeln!(stream, "{name}: {settings:?}")
                            .expect("writing to string never fails");
                    }
                }
            }
            "flush" => {
                const FLUSH: Signature = Signature {
                    cmd: "flush",
                    params: &[],
                };

                FLUSH.parser(input).finish()?;

                hypr.flush(Some(&mut stream.write_buf)).await?;
                stream.flush().await?;
            }
            inv_cmd => return Err(anyhow!("expected valid command, got `{inv_cmd}`")),
        }

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
