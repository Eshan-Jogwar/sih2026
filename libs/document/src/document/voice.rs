use async_trait::async_trait;
use orm::entity::document;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    document::base::Document,
    errors::DocumentErrors,
    storage::object_store::ObjectStore,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};

pub struct Voice<O: ObjectStore> {
    id: Uuid,
    key: String,
    storage: O,
}

impl<O: ObjectStore> Voice<O> {
    pub fn new(id: Uuid, key: String, storage: O) -> Self {
        Self { id, key, storage }
    }
}

#[async_trait]
impl<O: ObjectStore> Document for Voice<O> {
    fn id(&self) -> Uuid {
        self.id
    }

    fn mime_type(&self) -> &str {
        if self.key.ends_with(".wav") {
            "audio/wav"
        } else if self.key.ends_with(".mp3") {
            "audio/mpeg"
        } else if self.key.ends_with(".m4a") {
            "audio/mp4"
        } else if self.key.ends_with(".ogg") {
            "audio/ogg"
        } else if self.key.ends_with(".flac") {
            "audio/flac"
        } else {
            "audio/wav"
        }
    }

    async fn fetch(&self) -> Result<Vec<u8>, DocumentErrors> {
        self.storage.get(&self.key).await
    }

    async fn update_extracted_information(
        &self,
        json: Value,
        db: &DatabaseConnection,
    ) -> Result<(), DocumentErrors> {
        let result = document::Entity::find_by_id(self.id()).one(db).await;
        let result = result.map_err(DocumentErrors::DatabaseError)?;
        if result.is_none() {
            return Err(DocumentErrors::NotFound(String::from(
                "did not find the voice document",
            )));
        }
        let result = result.unwrap();
        let mut active_model: document::ActiveModel = result.into();
        active_model.extracted_information = Set(Some(json));
        active_model
            .update(db)
            .await
            .map_err(DocumentErrors::DatabaseError)?;

        Ok(())
    }
}
