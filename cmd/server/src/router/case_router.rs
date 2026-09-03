use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use document::document_manager::{CaseResponse, DocumentResponse};
use serde::Deserialize;
use uuid::Uuid;

use super::{AppError, AppState, MessageResponse};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateCaseRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCaseRequest {
    pub name: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/cases
async fn create_case_handler(
    State(manager): State<AppState>,
    Json(req): Json<CreateCaseRequest>,
) -> Result<impl IntoResponse, AppError> {
    let case: CaseResponse = manager.create_case(req.name).await?;
    Ok((StatusCode::CREATED, Json(case)))
}

/// GET /api/cases
async fn list_cases_handler(
    State(manager): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let cases: Vec<CaseResponse> = manager.list_cases().await?;
    Ok((StatusCode::OK, Json(cases)))
}

/// GET /api/cases/:id
async fn get_case_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let case: CaseResponse = manager.get_case(id).await?;
    Ok((StatusCode::OK, Json(case)))
}

/// PUT /api/cases/:id
async fn update_case_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCaseRequest>,
) -> Result<impl IntoResponse, AppError> {
    let case: CaseResponse = manager.update_case(id, req.name).await?;
    Ok((StatusCode::OK, Json(case)))
}

/// DELETE /api/cases/:id
async fn delete_case_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    manager.delete_case(id).await?;
    Ok((
        StatusCode::OK,
        Json(MessageResponse {
            message: format!("Case {} deleted", id),
        }),
    ))
}

/// GET /api/cases/:id/documents
async fn get_case_documents_handler(
    State(manager): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let docs: Vec<DocumentResponse> = manager.list_documents(Some(id)).await?;
    Ok((StatusCode::OK, Json(docs)))
}

// ---------------------------------------------------------------------------
// Router constructor
// ---------------------------------------------------------------------------

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/cases",
            get(list_cases_handler).post(create_case_handler),
        )
        .route(
            "/api/cases/{id}",
            get(get_case_handler)
                .put(update_case_handler)
                .delete(delete_case_handler),
        )
        .route("/api/cases/{id}/documents", get(get_case_documents_handler))
        .with_state(state)
}
