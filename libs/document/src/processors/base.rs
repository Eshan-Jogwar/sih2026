use crate::{document::base::Document, errors::DocumentErrors};
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use crate::storage::s3_object_store::S3ObjectStore;

#[async_trait]
pub trait DocumentProcessor: Send + Sync {
    /// Process a single document — extract information and persist it.
    async fn process(
        &self,
        doc: &dyn Document,
        db: &DatabaseConnection,
    ) -> Result<(), DocumentErrors>;

    /// Background cron: query DB for unprocessed documents, wrap them
    /// in the appropriate Document impl, and call self.process() on each.
    async fn cron_func(
        &self,
        db: &DatabaseConnection,
        store: &S3ObjectStore,
    ) -> Result<(), DocumentErrors>;
}
