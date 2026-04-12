//! `cm` binary entry point.
//!
//! Thin parse-and-dispatch shell. The clap surface lives in
//! [`cm_cli::cli::cli_def`]; admin handlers live in [`cm_cli::cli::admin`];
//! per-command handlers ship in the Read/Write phase sub-issues
//! (ALP-1774..ALP-1782) and are stubbed with `todo!()` until then.

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use cm_cli::cli::{self, Cli, Commands};

fn main() {
    if let Err(err) = run() {
        cli::errors::print_error(&err);
        std::process::exit(1);
    }
}

#[tokio::main]
async fn run() -> Result<()> {
    let cli_args = Cli::parse();

    // Hidden documentation flags. Both are `exclusive = true` so clap
    // rejects combining them with a subcommand. Handle them before
    // initializing tracing — the doc emitters write to stdout and we do
    // not want stray log lines on stderr in CI capture.
    if cli_args.markdown_help {
        print!("{}", clap_markdown::help_markdown::<Cli>());
        return Ok(());
    }
    if let Some(dir) = cli_args.generate_man_pages.as_deref() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating man page output directory {}", dir.display()))?;
        let cmd = Cli::command();
        clap_mangen::generate_to(cmd, dir).context("generating man pages")?;
        println!("wrote man pages to {}", dir.display());
        return Ok(());
    }

    // Initialize tracing (stderr only, never stdout: MCP uses stdout).
    let filter = if cli_args.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_writer(std::io::stderr)
        .init();

    match cli_args.command {
        // ---------------- READ ----------------
        Some(Commands::Recall {
            query,
            scope,
            kinds,
            tags,
            limit,
            max_tokens,
            json,
        }) => {
            let store = cli::open_store().await?;
            cli::recall::run(&store, query, scope, kinds, tags, limit, max_tokens, json).await?;
            if let Err(e) = cm_store::schema::wal_checkpoint(store.write_pool()).await {
                tracing::debug!(error = %e, "WAL checkpoint failed");
            }
            Ok(())
        }
        Some(Commands::Browse {
            scope_path,
            kind,
            tag,
            created_by,
            include_superseded,
            limit,
            cursor,
            json,
        }) => {
            let store = cli::open_store().await?;
            cli::browse::run(
                &store,
                scope_path,
                kind,
                tag,
                created_by,
                include_superseded,
                limit,
                cursor,
                json,
            )
            .await?;
            if let Err(e) = cm_store::schema::wal_checkpoint(store.write_pool()).await {
                tracing::debug!(error = %e, "WAL checkpoint failed");
            }
            Ok(())
        }
        Some(Commands::Get { ids, json }) => {
            let store = cli::open_store().await?;
            cli::get::run(&store, ids, json).await?;
            if let Err(e) = cm_store::schema::wal_checkpoint(store.write_pool()).await {
                tracing::debug!(error = %e, "WAL checkpoint failed");
            }
            Ok(())
        }
        Some(Commands::Stats { tag_sort, json }) => {
            let store = cli::open_store().await?;
            cli::stats::run(&store, tag_sort, json).await?;
            if let Err(e) = cm_store::schema::wal_checkpoint(store.write_pool()).await {
                tracing::debug!(error = %e, "WAL checkpoint failed");
            }
            Ok(())
        }

        // ---------------- WRITE ----------------
        Some(Commands::Store { .. }) => cli::store::run(),
        Some(Commands::Update {
            id,
            title,
            body,
            kind,
            meta,
            json,
        }) => {
            let store = cli::open_store().await?;
            cli::update::run(&store, id, title, body, kind, meta, json).await?;
            if let Err(e) = cm_store::schema::wal_checkpoint(store.write_pool()).await {
                tracing::debug!(error = %e, "WAL checkpoint failed");
            }
            Ok(())
        }
        Some(Commands::Deposit {
            exchanges,
            summary,
            scope_path,
            created_by,
            json,
        }) => {
            let store = cli::open_store().await?;
            cli::deposit::run(&store, exchanges, summary, scope_path, created_by, json).await?;
            if let Err(e) = cm_store::schema::wal_checkpoint(store.write_pool()).await {
                tracing::debug!(error = %e, "WAL checkpoint failed");
            }
            Ok(())
        }
        Some(Commands::Forget { ids }) => {
            let store = cli::open_store().await?;
            cli::forget::run(&store, ids).await?;
            if let Err(e) = cm_store::schema::wal_checkpoint(store.write_pool()).await {
                tracing::debug!(error = %e, "WAL checkpoint failed");
            }
            Ok(())
        }

        // ---------------- ADMIN ----------------
        Some(Commands::Init { global, force }) => cli::cmd_init(global, force),
        Some(Commands::Serve) => cli::cmd_serve().await,
        Some(Commands::Export { scope_path, format }) => {
            let store = cli::open_store().await?;
            cli::export::run(&store, scope_path, format).await?;
            if let Err(e) = cm_store::schema::wal_checkpoint(store.write_pool()).await {
                tracing::debug!(error = %e, "WAL checkpoint failed");
            }
            Ok(())
        }
        Some(Commands::Completions { shell }) => {
            use clap_complete::generate;
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "cm", &mut std::io::stdout());
            Ok(())
        }

        // No subcommand: show long help.
        None => {
            Cli::command().print_long_help()?;
            println!();
            Ok(())
        }
    }
}
