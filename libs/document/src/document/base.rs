use crate::errors::DocumentErrors;
use uuid::Uuid;

pub trait Document: Send + Sync {
    fn id(&self) -> Uuid;
    fn mime_type(&self) -> &str;
    fn validate(&self, bytes: &[u8]) -> Result<(), DocumentErrors>;
}
