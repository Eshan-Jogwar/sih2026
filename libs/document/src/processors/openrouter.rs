use async_trait::async_trait;
use base64::Engine;
use orm::entity::{document, sea_orm_active_enums::DocumentStatus};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Deserialize;

use crate::{
    document::{base::Document, image::Image},
    errors::DocumentErrors,
    processors::base::DocumentProcessor,
    storage::s3_object_store::S3ObjectStore,
};

/// OpenRouter vision-model processor.
///
/// Implements `DocumentProcessor` by sending document images to the
/// OpenRouter chat-completions API and persisting the extracted JSON.
pub struct OpenRouter {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenRouter {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }
}

// -- OpenRouter response deserialization types --------------------------------

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

// -- DocumentProcessor implementation -----------------------------------------

#[async_trait]
impl DocumentProcessor for OpenRouter {
    async fn process(
        &self,
        doc: &dyn Document,
        db: &DatabaseConnection,
    ) -> Result<(), DocumentErrors> {
        // 1. Fetch raw bytes from S3
        let bytes = doc.fetch().await?;

        // 2. Base64-encode
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

        // 3. Build OpenRouter vision request
        let mime = doc.mime_type();
        let data_url = format!("data:{};base64,{}", mime, b64);

        let payload = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "You are a document analysis AI. Extract ALL information from this document image into a structured JSON object. Include every field, name, date, number, address, and detail you can find. Also include a \"summary\" field with a brief summary of the document. Return ONLY valid JSON, no markdown fences."
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": data_url
                        }
                    }
                ]
            }]
        });

        // 4. POST to OpenRouter
        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(DocumentErrors::StorageError(format!(
                "OpenRouter API returned {} : {}",
                status, body
            )));
        }

        // 5. Parse response
        let completion: ChatCompletionResponse = response.json().await.map_err(|e| {
            DocumentErrors::StorageError(format!("Failed to parse OpenRouter response: {}", e))
        })?;

        let content = completion
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| {
                DocumentErrors::StorageError("No content in OpenRouter response".into())
            })?;

        // Strip markdown fences if the model wrapped the JSON
        let cleaned = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let extracted: serde_json::Value = serde_json::from_str(cleaned)
            .unwrap_or_else(|_| serde_json::json!({ "raw_text": content }));

        // 6. Persist extracted information
        doc.update_extracted_information(extracted, db).await?;

        Ok(())
    }

    async fn cron_func(
        &self,
        db: &DatabaseConnection,
        store: &S3ObjectStore,
    ) -> Result<(), DocumentErrors> {
        // Query for documents that are confirmed but not yet processed
        let documents = document::Entity::find()
            .filter(document::Column::ExtractedInformation.is_null())
            .filter(document::Column::Status.eq(DocumentStatus::Success))
            .all(db)
            .await
            .map_err(DocumentErrors::DatabaseError)?;

        if documents.is_empty() {
            return Ok(());
        }

        tracing::info!("Found {} unprocessed document(s)", documents.len());

        for model in documents {
            let doc_id = model.id;

            // Set status → Processing
            let mut active: document::ActiveModel = model.clone().into();
            active.status = Set(DocumentStatus::Processing);
            active.updated_at = Set(chrono::Utc::now().fixed_offset());
            if let Err(e) = active.update(db).await {
                tracing::error!("Failed to set Processing status for {}: {}", doc_id, e);
                continue;
            }

            // Wrap in Image (currently only image type is supported)
            let image = Image::new(doc_id, model.object_key.clone(), store.clone());

            // Process
            match self.process(&image, db).await {
                Ok(_) => {
                    // Set status → Finish
                    if let Ok(Some(doc)) = document::Entity::find_by_id(doc_id).one(db).await {
                        let mut active: document::ActiveModel = doc.into();
                        active.status = Set(DocumentStatus::Finish);
                        active.updated_at = Set(chrono::Utc::now().fixed_offset());
                        let _ = active.update(db).await;
                    }
                    tracing::info!("Successfully processed document {}", doc_id);
                }
                Err(e) => {
                    // Set status → Failed
                    if let Ok(Some(doc)) = document::Entity::find_by_id(doc_id).one(db).await {
                        let mut active: document::ActiveModel = doc.into();
                        active.status = Set(DocumentStatus::Failed);
                        active.updated_at = Set(chrono::Utc::now().fixed_offset());
                        let _ = active.update(db).await;
                    }
                    tracing::error!("Failed to process document {}: {}", doc_id, e);
                }
            }
        }

        Ok(())
    }
}
