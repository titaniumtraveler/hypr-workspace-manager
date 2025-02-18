use crate::{
    hypr::{Hypr, Workspace as HyprWorkspace},
    path_builder::PathBuilder,
    server::types::Request,
    socket::Socket,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap},
    fmt::Write,
    future::Future,
    io::{self, ErrorKind},
    path::Path,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::{
    fs::remove_file,
    io::{AsyncBufReadExt, Interest},
    net::{UnixListener, UnixSocket, UnixStream},
    select,
    sync::{mpsc, Notify, RwLock},
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
    state: State,
    receiver: Option<mpsc::Receiver<Socket>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    workspaces: HashMap<Arc<str>, WorkspaceSettings>,
    registers: BTreeMap<u8, Arc<str>>,
}

impl Server {
    pub const SOCKET: &'static str = "ws-mgr.sock";

    pub async fn setup_socket(path: &Path) -> Result<(Self, mpsc::Sender<Socket>, UnixListener)> {
        fn listener(path: &Path) -> io::Result<UnixListener> {
            let socket = UnixSocket::new_stream()?;

            socket2::SockRef::from(&socket).set_reuse_port(true)?;

            socket.bind(path)?;
            socket.listen(256)
        }

        let mut socket = match Socket::connect(path).await {
            Err(err) if err.kind() == ErrorKind::ConnectionRefused => {
                let (sender, receiver) = mpsc::channel(256);
                remove_file(path).await?;
                let listener = listener(path)?;
                let state = Server {
                    inner: RwLock::new(Inner {
                        state: Default::default(),
                        receiver: Some(receiver),
                    }),
                };
                return Ok((state, sender, listener));
            }
            r => r,
        }?;
        let listener = listener(path)?;

        socket.write_msg(&Request::Update)?;
        socket.flush().await?;

        let mut fd_count = 0;
        loop {
            todo!()
        }

        todo!("{fd_count}")
    }

    pub async fn recover_old_client(self: Arc<Self>, notify: Notify, mut stream: Socket) {}

    #[instrument(name = "socket server", err)]
    pub async fn run() -> Result<()> {
        let mut hypr_dir = PathBuilder::hypr_basepath()?;

        let hypr_path: Arc<Path> = hypr_dir.with_filename(".socket.sock").into();
        let (state, sender, socket) =
            Self::setup_socket(hypr_dir.with_filename(Self::SOCKET)).await?;
        let state = Arc::new(state);

        while let Ok((stream, _)) = socket.accept().await {
            tokio::spawn({
                let state = Arc::clone(&state);
                let sender = sender.clone();
                let hypr_path = Arc::clone(&hypr_path);

                async {
                    let res = state
                        .handle_client(Socket::from_unixstream(stream), sender, hypr_path)
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

    pub async fn try_upgrading_server(&self) -> Result<()> {
        Ok(())
    }

    pub async fn handle_client(
        self: Arc<Self>,
        mut stream: Socket,
        sender: mpsc::Sender<Socket>,
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
        let request: Request = stream.read_msg()?;
        debug!(?request, "input");
        match request {
            Request::Create { name } => {
                let mut lock = self.inner.write().await;
                match lock.state.workspaces.entry(name.into()) {
                    Entry::Vacant(vacant) => vacant.insert(WorkspaceSettings::default()),
                    Entry::Occupied(_) => return Err(anyhow!("name already in use")),
                };
            }
            Request::Bind { name, register } => {
                let mut lock = self.inner.write().await;
                let name = match lock.state.workspaces.get_key_value(name) {
                    Some((name, _)) => Arc::clone(name),
                    None => {
                        let name = Arc::from(name);
                        lock.state
                            .workspaces
                            .insert(Arc::clone(&name), WorkspaceSettings::default());
                        name
                    }
                };

                lock.state.registers.insert(register, name);
            }
            Request::Unbind { register } => {
                let mut lock = self.inner.write().await;
                lock.state.registers.remove(&register);
            }
            Request::Goto { register } => {
                let lock = self.inner.read().await;
                let name = lock.state.registers.get(&register).ok_or_else(|| {
                    anyhow!("register {register} does not point to any workspace")
                })?;

                hypr.go_to(HyprWorkspace::Name(name));
            }
            Request::Moveto { register } => {
                let lock = self.inner.read().await;
                let name = lock.state.registers.get(&register).ok_or_else(|| {
                    anyhow!("register {register} does not point to any workspace")
                })?;

                hypr.move_to(HyprWorkspace::Name(name));
            }
            Request::Read { workspace } => {
                match workspace {
                    Some(Workspace::Workspace(name)) => {
                        let lock = self.inner.read().await;
                        let (name, settings) =
                            lock.state.workspaces.get_key_value(name).ok_or_else(|| {
                                anyhow!("{name} doesn't point to any valid workspace")
                            })?;

                        stream.write_msg(&ReadResponse {
                            workspaces: IterMap::new([(name, settings)]),
                            registers: IterMap::new(
                                lock.state
                                    .registers
                                    .iter()
                                    .filter(|(_, register_pointee)| *register_pointee == name),
                            ),
                        })?;
                    }
                    Some(Workspace::Register(register)) => {
                        let lock = self.inner.read().await;
                        let name =
                            lock.state.registers.get(&register).ok_or_else(|| {
                                anyhow!("{register} does not point to any workspace")
                            })?;

                        let settings = lock.state.workspaces.get(name).ok_or_else(|| {
                            anyhow!("{name} doesn't point to any valid workspace")
                        })?;

                        stream.write_msg(&ReadResponse {
                            workspaces: IterMap::new([(name, settings)]),
                            registers: IterMap::new([(register, name)]),
                        })?;
                    }
                    None => {
                        let lock = self.inner.read().await;
                        stream.write_msg(&ReadResponse {
                            workspaces: &lock.state.workspaces,
                            registers: &lock.state.registers,
                        })?;
                    }
                }
            }
            Request::Update => todo!(),
            Request::Flush => {
                hypr.flush(Some(&mut stream.write_buf)).await?;
                stream.flush().await?;
            }
        }

        Ok(())
    }
}

struct WithCx<F>(F);

impl<F, O> Future for WithCx<F>
where
    F: FnMut(&mut Context) -> Poll<O>,
    F: Unpin,
{
    type Output = O;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0(cx)
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
