use std::sync::Arc;

use document::{document_manager::DocumentManager, storage::s3_object_store::S3ObjectStore};
use sea_orm::Database;

mod router;

#[tokio::main]
async fn main() {
    // Load .env
    let _ = dotenvy::dotenv();

    // Init tracing
    tracing_subscriber::fmt::init();

    // Read config from env
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let s3_bucket = std::env::var("S3_BUCKET_NAME").expect("S3_BUCKET_NAME must be set");
    let s3_region = std::env::var("S3_REGION").expect("S3_REGION must be set");
    let s3_endpoint = std::env::var("S3_ENDPOINT").expect("S3_ENDPOINT must be set");
    let s3_access_key = std::env::var("S3_ACCESS_KEY").expect("S3_ACCESS_KEY must be set");
    let s3_secret_key = std::env::var("S3_SECRET_KEY").expect("S3_SECRET_KEY must be set");
    let server_host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let server_port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "8000".to_string());

    // Connect to database
    tracing::info!("Connecting to database...");
    let db = Database::connect(&database_url)
        .await
        .expect("Failed to connect to database");
    tracing::info!("Database connected");

    // Create S3 object store
    let storage = S3ObjectStore::new(
        s3_bucket,
        s3_region,
        &s3_endpoint,
        s3_access_key,
        s3_secret_key,
    );

    // Create document manager
    let manager = Arc::new(DocumentManager::new(db, storage));

    // Build router
    let app = router::create_router(manager);

    // Start server
    let addr = format!("{}:{}", server_host, server_port);
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app).await.expect("Server error");
}
