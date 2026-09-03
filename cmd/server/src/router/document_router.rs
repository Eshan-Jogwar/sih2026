use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use document::document_manager::DocumentManager;
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub type AppState = Arc<DocumentManager>;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct InitiateUploadRequest {
    pub title: String,
    pub description: String,
    pub file_name: String,
    pub case_id: Uuid,
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

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

pub struct AppError(document::errors::DocumentErrors);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self.0 {
            document::errors::DocumentErrors::NotFound(msg) => {
                (StatusCode::NOT_FOUND, msg.clone())
            }
            document::errors::DocumentErrors::ValidationError(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            document::errors::DocumentErrors::StorageError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
            document::errors::DocumentErrors::NetworkError(e) => {
                (StatusCode::BAD_GATEWAY, e.to_string())
            }
            document::errors::DocumentErrors::DatabaseError(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        };

        tracing::error!("Request error: {}", message);
        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

impl From<document::errors::DocumentErrors> for AppError {
    fn from(err: document::errors::DocumentErrors) -> Self {
        AppError(err)
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/documents/upload/initiate
async fn initiate_upload_handler(
    State(manager): State<AppState>,
    Json(req): Json<InitiateUploadRequest>,
) -> Result<impl IntoResponse, AppError> {
    let response = manager
        .initiate_upload(req.title, req.description, req.file_name, req.case_id)
        .await?;

    Ok((StatusCode::OK, Json(response)))
}

/// POST /api/documents/upload/confirm
async fn confirm_upload_handler(
    State(manager): State<AppState>,
    Json(req): Json<ConfirmUploadRequest>,
) -> Result<impl IntoResponse, AppError> {
    let doc = manager
        .confirm_upload(req.document_id, req.success)
        .await?;

    Ok((StatusCode::OK, Json(doc)))
}

/// GET /api/documents/:id/download
async fn download_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let download_url = manager.get_download_url(id).await?;

    Ok((
        StatusCode::OK,
        Json(DownloadUrlResponse { download_url }),
    ))
}

/// GET /api/documents/:id
async fn get_document_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let doc = manager.get_document(id).await?;
    Ok((StatusCode::OK, Json(doc)))
}

/// GET /api/documents?case_id=...
async fn list_documents_handler(
    State(manager): State<AppState>,
    Query(params): Query<ListDocumentsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let docs = manager.list_documents(params.case_id).await?;
    Ok((StatusCode::OK, Json(docs)))
}

/// DELETE /api/documents/:id
async fn delete_document_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    manager.delete_document(id).await?;

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

/// Build the document router with all routes and middleware.
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/documents/upload/initiate", post(initiate_upload_handler))
        .route("/api/documents/upload/confirm", post(confirm_upload_handler))
        .route("/api/documents/{id}/download", get(download_handler))
        .route(
            "/api/documents/{id}",
            get(get_document_handler).delete(delete_document_handler),
        )
        .route("/api/documents", get(list_documents_handler))
        .layer(cors)
        .with_state(state)
}
