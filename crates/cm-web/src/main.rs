use anyhow::Result;
use clap::Parser;
use cm_web::{DEFAULT_PORT, ServeOptions};

#[derive(Parser)]
#[command(
    name = "cm-web",
    about = "Context-matters web monitoring interface",
    version
)]
struct Cli {
    /// Port to listen on
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Open browser after starting
    #[arg(long)]
    open: bool,

    /// Enable verbose debug output
    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let default_filter = if cli.verbose {
        "cm_web=debug,cm_store=debug,tower_http=debug"
    } else {
        "cm_web=warn,tower_http=warn"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!(verbose = cli.verbose);

    cm_web::serve(ServeOptions {
        open: cli.open,
        port: Some(cli.port),
        host: None,
    })
    .await
}
