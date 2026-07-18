use tokenwise_common::ExitCode;
use tokenwise_core::install::{Installer, ComponentStatus};

/// Install and configure the full 9-component tokenwise stack.
pub async fn run() -> Result<(), ExitCode> {
    let installer = Installer::new();
    match installer.run().await {
        Ok(summary) => {
            Installer::print_summary(&summary);
            let failed = summary.iter().filter(|(_, s)| matches!(s, ComponentStatus::Failed(_))).count();
            if failed > 0 {
                eprintln!("[WARN] {failed} component(s) failed to install. Run `tokenwise doctor` for details.");
                return Err(ExitCode::Failure);
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
