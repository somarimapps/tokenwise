use clap::{Parser, Subcommand};
use tokenwise_common::ExitCode;

mod commands;

/// tokenwise — 9-layer token optimization stack orchestrator for Claude Code and Hermes.
#[derive(Debug, Parser)]
#[command(name = "tokenwise", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Install tokenwise as an OS service (auto-start on login).
    Install,

    /// Register MCP servers and write rule files for an agent.
    Connect {
        /// Target agent to configure.
        #[arg(value_enum)]
        target: commands::connect::ConnectTarget,
    },

    /// Run 10-layer health check and report [PASS]/[WARN]/[FAIL] per layer.
    Doctor,

    /// Scan and repair broken hook command paths in settings.json.
    Sync,

    /// Show token savings statistics.
    Stats,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let cli = Cli::parse();

    let result: Result<(), ExitCode> = match cli.command {
        Command::Install => commands::install::run().await,
        Command::Connect { target } => commands::connect::run(target).await,
        Command::Doctor => commands::doctor::run().await,
        Command::Sync => commands::sync::run().await,
        Command::Stats => commands::stats::run().await,
    };

    match result {
        Ok(()) => std::process::exit(0),
        Err(code) => std::process::exit(code.into_code()),
    }
}
