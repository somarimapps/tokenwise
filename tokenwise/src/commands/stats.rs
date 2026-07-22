use tokenwise_common::ExitCode;
use tokenwise_core::stats::StatsAggregator;

/// Show token savings stats from all active stack layers.
pub async fn run() -> Result<(), ExitCode> {
    let result = StatsAggregator::collect().await;

    println!("{:<30} {}", "Layer", "Value");
    println!("{}", "─".repeat(60));

    let headroom = result
        .headroom_pct
        .map(|p| format!("{p:.1}% (proxy active on :8788)"))
        .unwrap_or_else(|| "— (proxy not active or using plugin)".to_string());
    println!("{:<30} {}", "Headroom savings", headroom);

    let rtk = match (result.rtk_calls, result.rtk_tokens_saved, result.rtk_savings_pct) {
        (Some(calls), Some(saved), Some(pct)) => {
            let saved_m = saved as f64 / 1_000_000.0;
            format!("{calls} commands  |  {saved_m:.2}M tokens saved  ({pct:.1}%)")
        }
        _ => "— (rtk not installed)".to_string(),
    };
    println!("{:<30} {}", "RTK (CLI layer)", rtk);

    let clawmem = result
        .clawmem_docs
        .map(|d| format!("{d} docs in vault"))
        .unwrap_or_else(|| "— (clawmem not installed)".to_string());
    println!("{:<30} {}", "ClawMem", clawmem);

    println!("{}", "─".repeat(60));

    let total = result
        .total_estimated_savings_pct
        .map(|p| {
            if result.headroom_pct.is_some() {
                format!("{p:.1}%  (measured end-to-end)")
            } else {
                format!("{p:.1}%  (RTK CLI layer only — headroom adds more)")
            }
        })
        .unwrap_or_else(|| "— (no data available)".to_string());
    println!("{:<30} {}", "Measured savings", total);

    Ok(())
}
