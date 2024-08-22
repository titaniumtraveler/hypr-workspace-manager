use hypr_workspace_manager::server::Server;
use std::sync::Arc;
use tracing::{error, info_span, Instrument};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let server = Arc::new(Server::default());
    async {
        server
            .run()
            .await
            .inspect_err(|err| error!(?err, "socket server failed to run"))
    }
    .instrument(info_span!("socket server"))
    .await?;

    Ok(())
}
