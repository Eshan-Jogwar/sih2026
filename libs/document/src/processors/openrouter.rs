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
        let doc_id = doc.id();
        tracing::info!("Fetching raw document bytes from storage for doc_id={}", doc_id);

        // 1. Fetch raw bytes from S3
        let bytes = doc.fetch().await?;
        let mime = doc.mime_type();
        tracing::info!(
            "Fetched {} bytes for doc_id={}, mime_type='{}'",
            bytes.len(),
            doc_id,
            mime
        );

        // 2. Base64-encode
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

        // 3. Build OpenRouter vision request
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
        tracing::info!(
            "Dispatching vision inference request for doc_id={} to OpenRouter (model: '{}')...",
            doc_id,
            self.model
        );
        let start_time = std::time::Instant::now();
        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let elapsed = start_time.elapsed();

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(
                "OpenRouter API error for doc_id={}: status={}, elapsed={:?}, body={}",
                doc_id,
                status,
                elapsed,
                body
            );
            return Err(DocumentErrors::StorageError(format!(
                "OpenRouter API returned {} : {}",
                status, body
            )));
        }

        tracing::info!(
            "OpenRouter API responded for doc_id={} in {:?} with status {}",
            doc_id,
            elapsed,
            response.status()
        );

        // 5. Parse response
        let completion: ChatCompletionResponse = response.json().await.map_err(|e| {
            tracing::error!("Failed to deserialize OpenRouter response JSON for doc_id={}: {}", doc_id, e);
            DocumentErrors::StorageError(format!("Failed to parse OpenRouter response: {}", e))
        })?;

        tracing::debug!(
            "OpenRouter returned {} choice(s) for doc_id={}",
            completion.choices.len(),
            doc_id
        );

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

        let extracted: serde_json::Value = match serde_json::from_str::<serde_json::Value>(cleaned) {
            Ok(val) => {
                if let Some(obj) = val.as_object() {
                    let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
                    tracing::info!("Extracted structured JSON for doc_id={} with keys: {:?}", doc_id, keys);
                } else {
                    tracing::info!("Extracted JSON value for doc_id={}", doc_id);
                }
                val
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to parse model response as JSON for doc_id={}: {}. Storing as raw_text.",
                    doc_id,
                    e
                );
                serde_json::json!({ "raw_text": content })
            }
        };

        // 6. Persist extracted information
        doc.update_extracted_information(extracted, db).await?;
        tracing::info!("Persisted extracted_information for doc_id={}", doc_id);

        Ok(())
    }

    async fn cron_func(
        &self,
        db: &DatabaseConnection,
        store: &S3ObjectStore,
    ) -> Result<(), DocumentErrors> {
        tracing::debug!("Document processor cron checking for unprocessed documents...");

        // Query for documents that are confirmed but not yet processed
        let documents = document::Entity::find()
            .filter(document::Column::ExtractedInformation.is_null())
            .filter(document::Column::Status.eq(DocumentStatus::Success))
            .all(db)
            .await
            .map_err(DocumentErrors::DatabaseError)?;

        if documents.is_empty() {
            tracing::debug!("Document processor cron: 0 unprocessed documents found.");
            return Ok(());
        }

        tracing::info!(
            "Document processor cron: Found {} unprocessed document(s) awaiting OpenRouter processing",
            documents.len()
        );

        for model in documents {
            let doc_id = model.id;
            tracing::info!(
                "Processing document id={}, title='{}', case_id={}",
                doc_id,
                model.title,
                model.case_id
            );

            // Set status → Processing
            let mut active: document::ActiveModel = model.clone().into();
            active.status = Set(DocumentStatus::Processing);
            active.updated_at = Set(chrono::Utc::now().fixed_offset());
            if let Err(e) = active.update(db).await {
                tracing::error!("Failed to set Processing status for doc_id={}: {}", doc_id, e);
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
                    tracing::info!("Successfully processed document id={}", doc_id);
                }
                Err(e) => {
                    // Set status → Failed
                    if let Ok(Some(doc)) = document::Entity::find_by_id(doc_id).one(db).await {
                        let mut active: document::ActiveModel = doc.into();
                        active.status = Set(DocumentStatus::Failed);
                        active.updated_at = Set(chrono::Utc::now().fixed_offset());
                        let _ = active.update(db).await;
                    }
                    tracing::error!("Failed to process document id={}: {}", doc_id, e);
                }
            }
        }

        Ok(())
    }
}
