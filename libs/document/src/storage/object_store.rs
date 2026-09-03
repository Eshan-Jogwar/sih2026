use crate::errors::DocumentErrors;
use async_trait::async_trait;
use std::time::Duration;

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put(&self, key: &str, data: Vec<u8>) -> Result<(), DocumentErrors>;
    async fn get(&self, key: &str) -> Result<Vec<u8>, DocumentErrors>;
    async fn delete(&self, key: &str) -> Result<(), DocumentErrors>;

    async fn presigned_put_url(
        &self,
        key: &str,
        expires_in: Duration,
    ) -> Result<String, DocumentErrors>;
    async fn presigned_get_url(
        &self,
        key: &str,
        expires_in: Duration,
    ) -> Result<String, DocumentErrors>;
}
