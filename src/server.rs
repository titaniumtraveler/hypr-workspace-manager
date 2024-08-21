use crate::{
    hypr::Hypr,
    path_builder::PathBuilder,
    server::signature::{Signature, Type},
};
use anyhow::{anyhow, Result};
use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap},
    env::VarError,
    fmt::{Debug, Write},
    io::ErrorKind,
    path::Path,
    str::from_utf8,
    sync::Arc,
};
use tokio::{
    fs::remove_file,
    io::{AsyncBufReadExt, AsyncWriteExt, BufStream},
    net::{unix::SocketAddr, UnixListener, UnixStream},
    sync::RwLock,
};

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
                return Err(anyhow!(
                    "expected to be started in the context of a running hyprland instance",
                ));
            }
            Err(VarError::NotUnicode(var)) => {
                return Err(anyhow!(
                    "invalid hyprland instance signature {var:?}, expected it to be unicode"
                ));
            }
        };

        let mut hypr_dir =
            PathBuilder::from_basepath(format_args!("/run/user/1000/hypr/{instance}"));

        let hypr_path: Arc<Path> = hypr_dir.with_filename(".socket.sock").into();
        let socket = hypr_dir.with_filename("ws-mgr.sock");
        if let Err(err) = remove_file(socket).await {
            if err.kind() != ErrorKind::NotFound {
                return Err(err.into());
            }
        }
        let socket = UnixListener::bind(socket)?;

        while let Ok((stream, socket)) = socket.accept().await {
            tokio::spawn(Self::handle_client(
                Arc::clone(&self),
                stream,
                socket,
                Arc::clone(&hypr_path),
            ));
        }

        Ok(())
    }

    pub async fn handle_client(
        self: Arc<Self>,
        stream: UnixStream,
        _: SocketAddr,
        hypr_path: Arc<Path>,
    ) -> Result<()> {
        let mut stream = BufStream::new(stream);
        let mut hypr = Hypr::new(&hypr_path);

        let mut input_buf = Vec::<u8>::new();
        let mut reply_buf = String::new();
        let mut err_buf = String::new();

        loop {
            input_buf.clear();
            stream.read_until(b'\n', &mut input_buf).await?;

            if input_buf.is_empty() {
                break;
            }

            if let Err(err) = self
                .handle_message(&mut stream, &mut hypr, &input_buf, &mut reply_buf)
                .await
            {
                err_buf.clear();
                err_buf.write_fmt(format_args!("{}", err))?;
                stream.write_all(err_buf.as_bytes()).await?;
                stream.flush().await?;
            }
        }

        hypr.flush(Some(&mut reply_buf)).await?;
        stream.write_all(reply_buf.as_ref()).await?;
        reply_buf.clear();

        Ok(())
    }

    pub async fn handle_message(
        &self,
        stream: &mut BufStream<UnixStream>,
        hypr: &mut Hypr,
        input: &[u8],
        reply: &mut String,
    ) -> Result<()> {
        let input = from_utf8(input)?;
        let (cmd, input) = Signature::parse_cmd(input).ok_or_else(|| anyhow!("expected param"))?;
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
            "go_to" => {
                const GO_TO: Signature = Signature {
                    cmd: "go_to",
                    params: &[("register", Type::U8)],
                };

                let mut parser = GO_TO.parser(input);
                let register: u8 = parser.parse_param()?;
                parser.finish()?;

                let lock = self.inner.read().await;
                let name = lock.registers.get(&register).ok_or_else(|| {
                    anyhow!("register {register} does not point to any workspace")
                })?;

                hypr.go_to(name);
            }
            "move_to" => {
                const MOVE_TO: Signature = Signature {
                    cmd: "move_to",
                    params: &[("register", Type::U8)],
                };

                let mut parser = MOVE_TO.parser(input);
                let register: u8 = parser.parse_param()?;
                parser.finish()?;

                let lock = self.inner.read().await;
                let name = lock.registers.get(&register).ok_or_else(|| {
                    anyhow!("register {register} does not point to any workspace")
                })?;

                hypr.move_to(name);
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
                        writeln!(reply, "{name}: {settings:?}")
                            .expect("writing to string never fails");
                    }
                } else {
                    for (name, settings) in lock.workspaces.iter() {
                        writeln!(reply, "{name}: {settings:?}")
                            .expect("writing to string never fails");
                    }
                }
                stream.write_all(reply.as_bytes()).await?;
                reply.clear();
            }
            "flush" => {
                const FLUSH: Signature = Signature {
                    cmd: "flush",
                    params: &[],
                };

                FLUSH.parser(input).finish()?;

                hypr.flush(Some(reply)).await?;
                stream.write_all(reply.as_bytes()).await?;
                reply.clear();
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
