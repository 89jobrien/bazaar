use crate::model::Project;

/// Port: any data source that yields a list of Projects.
#[async_trait::async_trait]
pub trait SourceFetcher: Send + Sync {
    async fn fetch(&self) -> anyhow::Result<Vec<Project>>;
}
