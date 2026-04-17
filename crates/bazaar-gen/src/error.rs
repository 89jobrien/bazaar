#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum BazaarError {
    #[error("HTTP error fetching {url}: {status}")]
    Http { url: String, status: u16 },
    #[error("Rate limited by {source} — retry after {retry_after:?}s")]
    RateLimited { source: Box<dyn std::error::Error + Send + Sync>, retry_after: Option<u64> },
    #[error("Render error: {0}")]
    Render(String),
}
