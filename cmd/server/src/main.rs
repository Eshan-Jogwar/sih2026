use std::sync::Arc;

use document::{
    document_manager::DocumentManager,
    processors::{base::DocumentProcessor, openrouter::OpenRouter},
    storage::s3_object_store::S3ObjectStore,
};
use sea_orm::Database;

mod router;

#[tokio::main]
async fn main() {
    // Load .env
    let _ = dotenvy::dotenv();

    // Init tracing with environment filter
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,server=debug,document=debug,tower_http=info".into());
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .init();

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
    tracing::info!("Database connected successfully");

    // Create S3 object store
    let storage = S3ObjectStore::new(
        s3_bucket,
        s3_region,
        &s3_endpoint,
        s3_access_key,
        s3_secret_key,
    );

    // Clone db and storage for the background processor
    let processor_db = db.clone();
    let processor_storage = storage.clone();

    // Create document manager
    let manager = Arc::new(DocumentManager::new(db, storage));

    // Background document processor cron configuration
    let enable_cron: bool = std::env::var("ENABLE_DOCUMENT_PROCESSOR_CRON")
        .map(|v| {
            let lower = v.trim().to_lowercase();
            lower == "true" || lower == "1" || lower == "yes"
        })
        .unwrap_or(true);

    if !enable_cron {
        tracing::info!("Document processor background cron is DISABLED (ENABLE_DOCUMENT_PROCESSOR_CRON=false).");
    } else {
        let cron_interval_secs: u64 = std::env::var("DOCUMENT_CRON_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        match std::env::var("OPENROUTER_API_KEY") {
            Ok(api_key) if !api_key.trim().is_empty() && !api_key.starts_with("your_") => {
                let model = std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| {
                    "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free".to_string()
                });
                tracing::info!(
                    "Starting background document processor cron with model '{}' (interval: {}s)...",
                    model,
                    cron_interval_secs
                );
                tokio::spawn(async move {
                    let processor = OpenRouter::new(api_key, model);
                    let mut interval =
                        tokio::time::interval(std::time::Duration::from_secs(cron_interval_secs));
                    loop {
                        interval.tick().await;
                        if let Err(e) = processor.cron_func(&processor_db, &processor_storage).await {
                            tracing::error!("Document processing cron error: {}", e);
                        }
                    }
                });
            }
            _ => {
                tracing::warn!(
                    "OPENROUTER_API_KEY is not set or is empty; background document processor cron will not run."
                );
            }
        }
    }

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
