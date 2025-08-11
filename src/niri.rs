use anyhow::{anyhow, Context};
use niri_ipc::{Action, Reply, Request, Response, WorkspaceReferenceArg as WorkspaceRef};
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

        serde_json::from_slice::<Reply>(&self.buffer)?.map_err(|err| anyhow!("niri-ipc err {err}"))
    }

    pub async fn goto(&mut self, name: &str) -> anyhow::Result<()> {
        let Response::Workspaces(workspaces) = self.request(&Request::Workspaces).await? else {
            return Err(anyhow!("expect Response::Workspaces"));
        };

        let workspace_exists = workspaces
            .iter()
            .filter_map(|workspace| workspace.name.as_deref())
            .any(|workspace_name| workspace_name == name);

        if !workspace_exists {
            self.request(&Request::Action(Action::SetWorkspaceName {
                name: name.to_owned(),
                workspace: Some(WorkspaceRef::Index(
                    workspaces
                        .iter()
                        .map(|workspace| workspace.idx)
                        .max()
                        .unwrap_or(1),
                )),
            }))
            .await?;
        }

        self.request(&Request::Action(Action::FocusWorkspace {
            reference: WorkspaceRef::Name(name.to_owned()),
        }))
        .await?;

        Ok(())
    }

    pub async fn moveto(&mut self, name: &str, focus: bool) -> anyhow::Result<()> {
        let Response::Workspaces(workspaces) = self.request(&Request::Workspaces).await? else {
            return Err(anyhow!("expect Response::Workspaces"));
        };

        let workspace_exists = workspaces
            .iter()
            .filter_map(|workspace| workspace.name.as_deref())
            .any(|workspace_name| workspace_name == name);

        if !workspace_exists {
            self.request(&Request::Action(Action::SetWorkspaceName {
                name: name.to_owned(),
                workspace: Some(WorkspaceRef::Index(
                    workspaces
                        .iter()
                        .map(|workspace| workspace.idx)
                        .max()
                        .unwrap_or(1),
                )),
            }))
            .await?;
        }

        self.request(&Request::Action(Action::MoveWindowToWorkspace {
            window_id: None,
            reference: WorkspaceRef::Name(name.to_owned()),
            focus,
        }))
        .await?;

        Ok(())
    }
}
