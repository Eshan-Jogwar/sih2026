use async_trait::async_trait;
use reqwest::Url;
use sea_orm::DatabaseConnection;
use serde_json::json;

use crate::{
    document::base::Document, errors::DocumentErrors, processors::base::DocumentProcessor,
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
        doc.update_extracted_information(json!(text), db).await;
        Ok(())
    }
}
