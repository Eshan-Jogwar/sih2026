use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use document::document_manager::{DocumentResponse, DocumentType, InitiateUploadResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AppError, AppState, MessageResponse};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct InitiateUploadRequest {
    pub title: String,
    pub description: String,
    pub file_name: String,
    pub case_id: Uuid,
    pub document_type: DocumentType,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmUploadRequest {
    pub document_id: Uuid,
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListDocumentsQuery {
    pub case_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct DownloadUrlResponse {
    pub download_url: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/documents/upload/initiate
async fn initiate_upload_handler(
    State(manager): State<AppState>,
    Json(req): Json<InitiateUploadRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(
        "Initiating upload: file='{}', title='{}', case_id={}, type={:?}",
        req.file_name,
        req.title,
        req.case_id,
        req.document_type
    );
    let response: InitiateUploadResponse = manager
        .initiate_upload(req.title, req.description, req.file_name, req.case_id, req.document_type)
        .await?;

    tracing::info!(
        "Upload initiated successfully: document_id={}, object_key='{}'",
        response.document_id,
        response.object_key
    );
    Ok((StatusCode::OK, Json(response)))
}

/// POST /api/documents/upload/confirm
async fn confirm_upload_handler(
    State(manager): State<AppState>,
    Json(req): Json<ConfirmUploadRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(
        "Confirming upload for document_id={}, success={}",
        req.document_id,
        req.success
    );
    let doc: DocumentResponse = manager.confirm_upload(req.document_id, req.success).await?;

    tracing::info!(
        "Upload confirmed: document_id={}, new_status={:?}",
        doc.id,
        doc.status
    );
    Ok((StatusCode::OK, Json(doc)))
}

/// GET /api/documents/:id/download
async fn download_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Generating presigned download URL for document_id={}", id);
    let download_url = manager.get_download_url(id).await?;

    Ok((StatusCode::OK, Json(DownloadUrlResponse { download_url })))
}

/// GET /api/documents/:id
async fn get_document_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Fetching document id={}", id);
    let doc: DocumentResponse = manager.get_document(id).await?;
    Ok((StatusCode::OK, Json(doc)))
}

/// GET /api/documents?case_id=...
async fn list_documents_handler(
    State(manager): State<AppState>,
    Query(params): Query<ListDocumentsQuery>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Listing documents with filter case_id={:?}", params.case_id);
    let docs: Vec<DocumentResponse> = manager.list_documents(params.case_id).await?;
    tracing::info!("Found {} document(s)", docs.len());
    Ok((StatusCode::OK, Json(docs)))
}

/// DELETE /api/documents/:id
async fn delete_document_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Deleting document id={}", id);
    manager.delete_document(id).await?;

    tracing::info!("Successfully deleted document id={}", id);
    Ok((
        StatusCode::OK,
        Json(MessageResponse {
            message: format!("Document {} deleted", id),
        }),
    ))
}

// ---------------------------------------------------------------------------
// Router constructor
// ---------------------------------------------------------------------------

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/documents/upload/initiate",
            post(initiate_upload_handler),
        )
        .route(
            "/api/documents/upload/confirm",
            post(confirm_upload_handler),
        )
        .route("/api/documents/{id}/download", get(download_handler))
        .route(
            "/api/documents/{id}",
            get(get_document_handler).delete(delete_document_handler),
        )
        .route("/api/documents", get(list_documents_handler))
        .with_state(state)
}
