use std::time::Duration;

use orm::entity::document::{self, ActiveModel, Entity as DocumentEntity, Model as DocumentModel};
use orm::entity::sea_orm_active_enums::DocumentStatus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::DocumentErrors;
use crate::storage::object_store::ObjectStore;
use crate::storage::s3_object_store::S3ObjectStore;

/// Response returned when a new upload is initiated.
#[derive(Debug, Serialize, Deserialize)]
pub struct InitiateUploadResponse {
    pub document_id: Uuid,
    pub upload_url: String,
    pub object_key: String,
}

/// Orchestrates document lifecycle: upload initiation, confirmation,
/// download URL generation, listing, and deletion.
pub struct DocumentManager {
    db: DatabaseConnection,
    storage: S3ObjectStore,
}

impl DocumentManager {
    /// Create a new `DocumentManager`.
    pub fn new(db: DatabaseConnection, storage: S3ObjectStore) -> Self {
        Self { db, storage }
    }

    /// Initiate a new document upload.
    ///
    /// 1. Generates a unique S3 object key: `{new_uuid}/{file_name}`
    /// 2. Inserts a database record with status = Pending
    /// 3. Generates a presigned PUT URL (5 min expiry)
    /// 4. Returns the document ID, upload URL, and object key
    pub async fn initiate_upload(
        &self,
        title: String,
        description: String,
        file_name: String,
        case_id: Uuid,
    ) -> Result<InitiateUploadResponse, DocumentErrors> {
        let doc_id = Uuid::new_v4();
        let object_key = format!("{}/{}", doc_id, file_name);

        // Insert document record with Pending status
        let now = chrono::Utc::now().fixed_offset();
        let active_model = ActiveModel {
            id: Set(doc_id),
            title: Set(title),
            description: Set(description),
            status: Set(DocumentStatus::Pending),
            object_key: Set(object_key.clone()),
            extracted_information: Set(None),
            case_id: Set(case_id),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active_model.insert(&self.db).await?;

        // Generate presigned PUT URL (5 minutes)
        let upload_url = self
            .storage
            .presigned_put_url(&object_key, Duration::from_secs(300))
            .await?;

        Ok(InitiateUploadResponse {
            document_id: doc_id,
            upload_url,
            object_key,
        })
    }

    /// Confirm (or fail) a previously initiated upload.
    ///
    /// Updates the document status from Pending to Success or Failed
    /// based on the `success` flag from the frontend.
    pub async fn confirm_upload(
        &self,
        document_id: Uuid,
        success: bool,
    ) -> Result<DocumentModel, DocumentErrors> {
        let doc = DocumentEntity::find_by_id(document_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                DocumentErrors::NotFound(format!("Document {} not found", document_id))
            })?;

        if doc.status != DocumentStatus::Pending {
            return Err(DocumentErrors::ValidationError(format!(
                "Document {} is not in Pending status (current: {:?})",
                document_id, doc.status
            )));
        }

        let new_status = if success {
            DocumentStatus::Success
        } else {
            DocumentStatus::Failed
        };

        let now = chrono::Utc::now().fixed_offset();
        let mut active_model: ActiveModel = doc.into();
        active_model.status = Set(new_status);
        active_model.updated_at = Set(now);

        let updated = active_model.update(&self.db).await?;
        Ok(updated)
    }

    /// Generate a presigned download (GET) URL for a document.
    pub async fn get_download_url(
        &self,
        document_id: Uuid,
    ) -> Result<String, DocumentErrors> {
        let doc = DocumentEntity::find_by_id(document_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                DocumentErrors::NotFound(format!("Document {} not found", document_id))
            })?;

        let download_url = self
            .storage
            .presigned_get_url(&doc.object_key, Duration::from_secs(300))
            .await?;

        Ok(download_url)
    }

    /// Get a single document by ID.
    pub async fn get_document(
        &self,
        document_id: Uuid,
    ) -> Result<DocumentModel, DocumentErrors> {
        DocumentEntity::find_by_id(document_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                DocumentErrors::NotFound(format!("Document {} not found", document_id))
            })
    }

    /// List documents, optionally filtered by case ID.
    pub async fn list_documents(
        &self,
        case_id: Option<Uuid>,
    ) -> Result<Vec<DocumentModel>, DocumentErrors> {
        let mut query = DocumentEntity::find();

        if let Some(cid) = case_id {
            query = query.filter(document::Column::CaseId.eq(cid));
        }

        let docs = query.all(&self.db).await?;
        Ok(docs)
    }

    /// Delete a document from both S3 and the database.
    pub async fn delete_document(
        &self,
        document_id: Uuid,
    ) -> Result<(), DocumentErrors> {
        let doc = DocumentEntity::find_by_id(document_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                DocumentErrors::NotFound(format!("Document {} not found", document_id))
            })?;

        // Delete from S3 first
        self.storage.delete(&doc.object_key).await?;

        // Then delete from database
        let active_model: ActiveModel = doc.into();
        active_model.delete(&self.db).await?;

        Ok(())
    }
}
