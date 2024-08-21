use hypr_workspace_manager::server::Server;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let server = Arc::new(Server::default());
    server.run().await?;

    Ok(())
}
