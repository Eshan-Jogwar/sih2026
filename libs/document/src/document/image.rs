use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::{document::base::Document, errors::DocumentErrors, storage::object_store::ObjectStore};
use sea_orm::DatabaseConnection;

pub struct Image<O: ObjectStore> {
    id: Uuid,
    key: String,
    storage: O,
}

#[async_trait]
impl<O: ObjectStore> Document for Image<O> {
    fn id(&self) -> Uuid {
        self.id
    }

    fn mime_type(&self) -> &str {
        "image/jpg"
    }

    async fn fetch(&self) -> Result<Vec<u8>, DocumentErrors> {
        self.storage.get(&self.key).await
    }

    async fn update_extracted_information(
        &self,
        json: Value,
        db: &DatabaseConnection,
    ) -> Result<(), DocumentErrors> {
        Ok(())
    }
}
