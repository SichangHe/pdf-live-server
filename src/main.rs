use pdf_live_server::run;
use tracing::Level;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy(),
        )
        .init();

    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?
        .block_on(run())
}
