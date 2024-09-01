use clap::Parser;
use hypr_workspace_manager::cli::Cli;

#[allow(dead_code)]
fn tracing_flat() {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::FULL)
        .init();
}

#[allow(dead_code)]
fn tracing_tree() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};
    use tracing_tree::HierarchicalLayer;
    Registry::default().with(HierarchicalLayer::new(4)).init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_tree();
    Cli::parse().run().await?;

    Ok(())
}
