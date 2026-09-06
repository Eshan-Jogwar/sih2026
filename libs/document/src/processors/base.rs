use crate::{document::base::Document, errors::DocumentErrors};
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use crate::storage::s3_object_store::S3ObjectStore;
#[async_trait]
pub trait DocumentProcessor: Send + Sync {
    async fn process(
        &self,
        doc: impl Document,
        db: &DatabaseConnection,
    ) -> Result<(), DocumentErrors>;
}
