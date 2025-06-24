use crate::{niri::Niri, server::types::Request, socket::Socket};
use anyhow::{anyhow, Result};
use niri_ipc::{
    Action::FocusWorkspace, Request as NiriRequest, WorkspaceReferenceArg as WorkspaceRef,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap},
    io::ErrorKind,
    sync::Arc,
};
use tokio::{
    fs::remove_file,
    net::{unix::SocketAddr, UnixListener},
    sync::RwLock,
};
use tracing::{debug, error, info, info_span, instrument, warn, Instrument};
use types::{util::IterMap, ReadResponse, Workspace};

pub mod types;

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
    pub const SOCKET: &'static str = "ws-mgr.sock";

    #[instrument(name = "socket server", skip(self), err)]
    pub async fn run(self: Arc<Self>) -> Result<()> {
        let socket = crate::socket::socket_path()?;

        if let Err(err) = remove_file(&socket).await {
            if err.kind() != ErrorKind::NotFound {
                return Err(err.into());
            }
        }
        let socket = UnixListener::bind(&socket)?;

        while let Ok((stream, socket)) = socket.accept().await {
            tokio::spawn({
                let server_state = Arc::clone(&self);

                async {
                    let res = server_state
                        .handle_client(Socket::from_unixstream(stream), socket)
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

    pub async fn handle_client(self: Arc<Self>, mut stream: Socket, _: SocketAddr) -> Result<()> {
        info!("connected");

        let mut niri = Niri::from_env().await?;

        loop {
            let res = async {
                debug!("waiting for input");
                if !stream.fetch_msg().await? {
                    return Ok(false);
                }

                if let Err(err) = self.handle_message(&mut stream, &mut niri).await {
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

        stream.flush().await?;

        info!("disconnected");

        Ok(())
    }

    pub async fn handle_message(&self, stream: &mut Socket, niri: &mut Niri) -> Result<()> {
        let request: Request = stream.read_msg()?;
        debug!(?request, "input");
        match request {
            Request::Create { name } => {
                let mut lock = self.inner.write().await;
                match lock.workspaces.entry(name.into()) {
                    Entry::Vacant(vacant) => vacant.insert(WorkspaceSettings::default()),
                    Entry::Occupied(_) => return Err(anyhow!("name already in use")),
                };
            }
            Request::Bind { name, register } => {
                let mut lock = self.inner.write().await;
                let name = match lock.workspaces.get_key_value(name) {
                    Some((name, _)) => Arc::clone(name),
                    None => {
                        let name = Arc::from(name);
                        lock.workspaces
                            .insert(Arc::clone(&name), WorkspaceSettings::default());
                        name
                    }
                };

                lock.registers.insert(register, name);
            }
            Request::Unbind { register } => {
                let mut lock = self.inner.write().await;
                lock.registers.remove(&register);
            }
            Request::Goto { register } => {
                let lock = self.inner.read().await;
                let name = lock.registers.get(&register).ok_or_else(|| {
                    anyhow!("register {register} does not point to any workspace")
                })?;

                niri.request(&NiriRequest::Action(FocusWorkspace {
                    reference: WorkspaceRef::Name(name.as_ref().to_owned()),
                }))
                .await?;
            }
            Request::Moveto { register } => {
                let lock = self.inner.read().await;
                let name = lock.registers.get(&register).ok_or_else(|| {
                    anyhow!("register {register} does not point to any workspace")
                })?;

                niri.request(&NiriRequest::Action(
                    niri_ipc::Action::MoveWindowToWorkspace {
                        window_id: None,
                        reference: WorkspaceRef::Name(name.as_ref().to_owned()),
                        focus: false,
                    },
                ))
                .await?;
            }
            Request::Read { workspace } => match workspace {
                Some(Workspace::Workspace(name)) => {
                    let guard = self.inner.read().await;
                    let (name, settings) = guard
                        .workspaces
                        .get_key_value(name)
                        .ok_or_else(|| anyhow!("{name} doesn't point to any valid workspace"))?;

                    stream.write_msg(&ReadResponse {
                        workspaces: IterMap::new([(name, settings)]),
                        registers: IterMap::new(
                            guard
                                .registers
                                .iter()
                                .filter(|(_, register_pointee)| *register_pointee == name),
                        ),
                    })?;
                }
                Some(Workspace::Register(register)) => {
                    let guard = self.inner.read().await;
                    let name = guard
                        .registers
                        .get(&register)
                        .ok_or_else(|| anyhow!("{register} does not point to any workspace"))?;

                    let settings = guard
                        .workspaces
                        .get(name)
                        .ok_or_else(|| anyhow!("{name} doesn't point to any valid workspace"))?;

                    stream.write_msg(&ReadResponse {
                        workspaces: IterMap::new([(name, settings)]),
                        registers: IterMap::new([(register, name)]),
                    })?;
                }
                None => {
                    let guard = self.inner.read().await;
                    stream.write_msg(&ReadResponse {
                        workspaces: IterMap::new(&guard.workspaces),
                        registers: IterMap::new(&guard.registers),
                    })?;
                }
            },
            Request::Flush => {
                stream.flush().await?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkspaceSettings {}

#[allow(clippy::derivable_impls)]
impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {}
    }
}
