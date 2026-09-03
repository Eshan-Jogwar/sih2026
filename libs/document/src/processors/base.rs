use crate::{document::base::Document, errors::DocumentErrors};
use async_trait::async_trait;

#[async_trait]
pub trait DocumentProcessor: Send + Sync {
    async fn process(&self, doc: impl Document, bytes: &[u8]) -> Result<(), DocumentErrors>;
}
