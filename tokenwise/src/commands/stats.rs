use tokenwise_common::ExitCode;
use tokenwise_core::stats::StatsAggregator;

/// Show token savings stats from all active stack layers.
pub async fn run() -> Result<(), ExitCode> {
    let result = StatsAggregator::collect().await;

    println!("{:<30} Value", "Layer");
    println!("{}", "─".repeat(50));

    let headroom = result
        .headroom_pct
        .map(|p| format!("{p:.1}%"))
        .unwrap_or_else(|| "—".to_string());
    println!("{:<30} {}", "Headroom savings", headroom);

    let rtk = result
        .rtk_calls
        .map(|c| c.to_string())
        .unwrap_or_else(|| "—".to_string());
    println!("{:<30} {}", "RTK proxy calls", rtk);

    let clawmem = result
        .clawmem_docs
        .map(|d| d.to_string())
        .unwrap_or_else(|| "—".to_string());
    println!("{:<30} {}", "ClawMem docs", clawmem);

    let total = result
        .total_estimated_savings_pct
        .map(|p| format!("{p:.1}%"))
        .unwrap_or_else(|| "—".to_string());
    println!("{}", "─".repeat(50));
    println!("{:<30} {}", "Estimated total savings", total);

    Ok(())
}
