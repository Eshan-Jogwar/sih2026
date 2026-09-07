use std::time::Duration;

use reqwest::Client;
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

use crate::{errors::DocumentErrors, storage::object_store::ObjectStore};

#[derive(Clone)]
pub struct S3ObjectStore {
    bucket: Bucket,
    credentials: Credentials,
    client: Client,
}

impl S3ObjectStore {
    pub fn new(
        bucket_name: String,
        region: String,
        endpoint: &str,
        access_key: String,
        secret_key: String,
    ) -> Self {
        let endpoint = endpoint.parse().expect("Invalid S3 endpoint URL");

        let bucket = Bucket::new(endpoint, UrlStyle::Path, bucket_name, region)
            .expect("Failed to create S3 bucket handle");

        let credentials = Credentials::new(access_key, secret_key);

        let client = Client::new();

        Self {
            bucket,
            credentials,
            client,
        }
    }
}

#[async_trait::async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put(&self, key: &str, data: Vec<u8>) -> Result<(), DocumentErrors> {
        let url = self.presigned_put_url(key, Duration::from_secs(60)).await?;

        let response = self.client.put(&url).body(data).send().await?;

        if !response.status().is_success() {
            return Err(DocumentErrors::StorageError(format!(
                "S3 PUT failed with status {}",
                response.status()
            )));
        }

        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, DocumentErrors> {
        let url = self.presigned_get_url(key, Duration::from_secs(60)).await?;

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(DocumentErrors::StorageError(format!(
                "S3 GET failed with status {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn delete(&self, key: &str) -> Result<(), DocumentErrors> {
        let action = self.bucket.delete_object(Some(&self.credentials), key);
        let url = action.sign(Duration::from_secs(60));

        let response = self.client.delete(url.as_str()).send().await?;

        if !response.status().is_success() {
            return Err(DocumentErrors::StorageError(format!(
                "S3 DELETE failed with status {}",
                response.status()
            )));
        }

        Ok(())
    }

    async fn presigned_put_url(
        &self,
        key: &str,
        expires_in: Duration,
    ) -> Result<String, DocumentErrors> {
        let action = self.bucket.put_object(Some(&self.credentials), key);
        Ok(action.sign(expires_in).to_string())
    }

    async fn presigned_get_url(
        &self,
        key: &str,
        expires_in: Duration,
    ) -> Result<String, DocumentErrors> {
        let action = self.bucket.get_object(Some(&self.credentials), key);
        Ok(action.sign(expires_in).to_string())
    }
}
