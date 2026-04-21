use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageDay {
    pub date: String,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u64,
    #[serde(rename = "totalCost")]
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageTotals {
    #[serde(rename = "totalCost")]
    pub total_cost: f64,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u64,
    #[serde(rename = "inputTokens")]
    pub input_tokens: u64,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    #[serde(rename = "cacheCreationTokens")]
    pub cache_creation_tokens: u64,
    #[serde(rename = "cacheReadTokens")]
    pub cache_read_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub totals: UsageTotals,
    pub daily: Vec<UsageDay>,
}

impl UsageSnapshot {
    pub fn peak_day(&self) -> Option<&UsageDay> {
        self.daily
            .iter()
            .max_by(|a, b| a.total_cost.partial_cmp(&b.total_cost).unwrap())
    }
}
