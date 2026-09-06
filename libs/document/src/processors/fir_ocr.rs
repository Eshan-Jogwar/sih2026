use async_trait::async_trait;
use orm::entity::{document, sea_orm_active_enums::DocumentType};
use reqwest::Url;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::json;

use crate::{
    document::{base::Document, image::Image}, errors::DocumentErrors, processors::base::DocumentProcessor, storage::s3_object_store::S3ObjectStore,
};

pub struct FirOcr {
    model_endpoint: Url,
}

impl FirOcr {
    async fn call_api_get_text(&self, _file: Vec<u8>) -> String {
        self.model_endpoint.to_string() + ": ocr output"
    }
}

#[async_trait]
impl DocumentProcessor for FirOcr {
    async fn process(
        &self,
        doc: impl Document,
        db: &DatabaseConnection,
    ) -> Result<(), DocumentErrors> {
        let file = doc.fetch().await?;
        let text = self.call_api_get_text(file).await;
        doc.update_extracted_information(json!(text), db).await?;
        Ok(())
    }

    async fn cron_fuc(
        &self,
        db: &DatabaseConnection,
        store: &S3ObjectStore
    ) {
        let documents = document::Entity::find()
            .filter(document::Column::ExtractedInformation.is_null())
            .filter(document::Column::Type.eq(DocumentType::Image))
            .all(db)
            .await;

        if documents.is_ok() {
            let documents = documents.unwrap();
            for model in documents {
                self.process(Image {}, db);
            }
        }

    }
}
