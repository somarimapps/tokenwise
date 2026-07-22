use adapter_claude::ClaudeConnector;
use tokenwise_common::ExitCode;
use tokenwise_core::install::{ComponentStatus, Installer};

/// Install and configure the full 9-component tokenwise stack, then auto-connect Claude Code.
pub async fn run() -> Result<(), ExitCode> {
    let installer = Installer::new();
    match installer.run().await {
        Ok(summary) => {
            Installer::print_summary(&summary);
            let failed = summary
                .iter()
                .filter(|(_, s)| matches!(s, ComponentStatus::Failed(_)))
                .count();
            if failed > 0 {
                eprintln!(
                    "[WARN] {failed} component(s) failed to install. Run `tokenwise doctor` for details."
                );
                return Err(ExitCode::Failure);
            }
            // Auto-connect Claude Code so the user never has to run `tokenwise connect claude` manually.
            println!("Connecting Claude Code to tokenwise stack...");
            match ClaudeConnector::new().and_then(|c| c.connect(false)) {
                Ok(()) => println!("Claude Code connected to tokenwise stack."),
                Err(e) => eprintln!(
                    "[WARN] Auto-connect failed: {e}. Run `tokenwise connect claude` to connect manually."
                ),
            }
            println!("Installation complete. Run `tokenwise doctor` to verify the stack.");
            Ok(())
        }
        Err(e) => {
            eprintln!("[FAIL] {e}");
            Err(ExitCode::Failure)
        }
    }
}
