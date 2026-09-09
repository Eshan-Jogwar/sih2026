use async_trait::async_trait;
use orm::entity::document;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use serde_json::Value;
use uuid::Uuid;

use crate::{document::base::Document, errors::DocumentErrors};

/// An in-memory text document representation created from image OCR or pipeline transcription.
/// Enables passing transcribed image text directly to `NerProcessor::process` without
/// writing partial state to the database first.
pub struct TranscribedDocument {
    id: Uuid,
    transcribed_text: String,
}

impl TranscribedDocument {
    pub fn new(id: Uuid, transcribed_text: String) -> Self {
        Self {
            id,
            transcribed_text,
        }
    }
}

#[async_trait]
impl Document for TranscribedDocument {
    fn id(&self) -> Uuid {
        self.id
    }

    fn mime_type(&self) -> &str {
        "text/plain"
    }

    async fn fetch(&self) -> Result<Vec<u8>, DocumentErrors> {
        Ok(self.transcribed_text.as_bytes().to_vec())
    }

    async fn update_extracted_information(
        &self,
        mut json: Value,
        db: &DatabaseConnection,
    ) -> Result<(), DocumentErrors> {
        // Ensure transcribed_text is preserved in the persisted JSON
        if let Value::Object(ref mut map) = json {
            if !map.contains_key("transcribed_text") {
                map.insert(
                    "transcribed_text".to_string(),
                    Value::String(self.transcribed_text.clone()),
                );
            }
        }

        let doc = document::Entity::find_by_id(self.id)
            .one(db)
            .await
            .map_err(DocumentErrors::DatabaseError)?
            .ok_or_else(|| {
                DocumentErrors::NotFound(format!("Document {} not found in database", self.id))
            })?;

        let mut active: document::ActiveModel = doc.into();
        active.extracted_information = Set(Some(json));
        active.update(db).await.map_err(DocumentErrors::DatabaseError)?;

        Ok(())
    }
}
