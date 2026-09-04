use crate::errors::DocumentErrors;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

#[async_trait]
pub trait Document: Send + Sync {
    fn id(&self) -> Uuid;
    fn mime_type(&self) -> &str;
    async fn fetch(&self) -> Result<Vec<u8>, DocumentErrors>;
    async fn update_extracted_information(
        &self,
        json: Value,
        db: &DatabaseConnection,
    ) -> Result<(), DocumentErrors>;
}
