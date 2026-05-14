use crate::model::{UsageDay, UsageSnapshot, UsageTotals};
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

/// Raw ccusage --json shape (daily array field is "daily" in our snapshot,
/// but ccusage emits it as the top-level array under a key we need to map).
#[derive(Deserialize)]
struct RawSnapshot {
    daily: Option<Vec<RawDay>>,
    totals: UsageTotals,
}

#[derive(Deserialize)]
struct RawDay {
    date: String,
    #[serde(rename = "totalTokens")]
    total_tokens: u64,
    #[serde(rename = "totalCost")]
    total_cost: f64,
}

pub fn load_usage(path: &Path) -> Result<Option<UsageSnapshot>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let snap: RawSnapshot = serde_json::from_str(&raw)?;
    let daily = snap
        .daily
        .unwrap_or_default()
        .into_iter()
        .map(|d| UsageDay {
            date: d.date,
            total_tokens: d.total_tokens,
            total_cost: d.total_cost,
        })
        .collect();
    Ok(Some(UsageSnapshot {
        totals: snap.totals,
        daily,
    }))
}
