use document::storage::{object_store::ObjectStore, s3_object_store::S3ObjectStore};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let store = S3ObjectStore::new(
        "bucket".to_string(),
        "us-east-1".to_string(),
        "http://localhost:9000",
        "2UAQyY3L5dlvDNDV83F3".to_string(),
        "EhQKgnFnOlVs747cimPso5DRUhFzBC2VcYCZm6if".to_string(),
    );

    let key = "a.png";

    let put_url = store
        .presigned_put_url(key, Duration::from_secs(300))
        .await
        .expect("failed to generate PUT URL");

    let get_url = store
        .presigned_get_url(key, Duration::from_secs(300))
        .await
        .expect("failed to generate GET URL");

    println!("PUT URL:\n{}\n", put_url);
    println!("GET URL:\n{}\n", get_url);
}
