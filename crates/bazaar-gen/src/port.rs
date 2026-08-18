use crate::model::Project;
use std::path::Path;

/// Port: any data source that yields a list of Projects.
#[async_trait::async_trait]
pub trait SourceFetcher: Send + Sync {
    async fn fetch(&self) -> anyhow::Result<Vec<Project>>;
}

/// Port: anything that can run an enrichment pipeline given a spec file and
/// JSON input, returning structured JSON output. Abstracts the LLM boundary
/// so enrichment logic never depends on a concrete subprocess/API call.
pub trait PipelineRunner: Send + Sync {
    fn run(&self, pipeline: &Path, input_json: &str) -> anyhow::Result<serde_json::Value>;
}
