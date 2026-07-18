use adapter_claude::ClaudeConnector;
use adapter_hermes::HermesConnector;
use tokenwise_common::ExitCode;

/// Target agent to connect.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ConnectTarget {
    Claude,
    Hermes,
}

/// Register MCP servers and write rule files for the given target.
pub async fn run(target: ConnectTarget) -> Result<(), ExitCode> {
    match target {
        ConnectTarget::Claude => {
            let connector = ClaudeConnector::new().map_err(|e| {
                eprintln!("[FAIL] {e}");
                ExitCode::Failure
            })?;
            connector.connect(false).map_err(|e| {
                eprintln!("[FAIL] {e}");
                ExitCode::Failure
            })?;
            println!("Claude Code connected to tokenwise stack.");
        }
        ConnectTarget::Hermes => {
            let connector = HermesConnector::new().map_err(|e| {
                eprintln!("[FAIL] {e}");
                ExitCode::Failure
            })?;
            connector.connect().map_err(|e| {
                eprintln!("[FAIL] {e}");
                ExitCode::Failure
            })?;
            println!("Hermes agent connected to tokenwise stack.");
        }
    }
    Ok(())
}
