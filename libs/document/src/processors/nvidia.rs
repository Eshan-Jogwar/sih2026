use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use orm::entity::{
    document,
    sea_orm_active_enums::{DocumentStatus, DocumentType},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde::Deserialize;

use crate::{
    document::{base::Document, image::Image, text::Text, voice::Voice},
    errors::DocumentErrors,
    processors::base::DocumentProcessor,
    storage::s3_object_store::S3ObjectStore,
};

// ---------------------------------------------------------------------------
// Specialized Forensic Prompts
// ---------------------------------------------------------------------------

const IMAGE_OCR_PROMPT: &str = r#"You are an expert forensic document and crime scene evidence analysis AI.
Analyze this document or evidence image thoroughly and extract all information into a structured JSON object.

Include the following fields:
1. "transcribed_text": Extract ALL visible and handwritten text verbatim (including Hindi/Devanagari and English), preserving paragraph structure and field labels.
2. "summary": A concise executive summary of what this document or image shows.
3. "document_classification": The specific type of document (e.g., "FIR", "General Diary", "Case Diary", "Forensic Photo", "Seizure Memo", "Identity Card", "Financial Slip").
4. "entities": Extract all identifiable entities:
   - "persons": [ {"name": "...", "role": "suspect"|"accused"|"victim"|"witness"|"informant"|"police"|"other", "alias": "..."} ]
   - "phone_numbers": [ {"number": "...", "owner": "...", "context": "..."} ]
   - "financial_accounts": [ {"account_number": "...", "bank": "...", "holder": "..."} ]
   - "vehicles": [ {"registration_number": "...", "make_model": "...", "color": "..."} ]
   - "weapons_objects": [ {"item": "...", "description": "..."} ]
   - "locations": [ {"place": "...", "address": "...", "significance": "..."} ]
   - "dates_times": [ {"date": "...", "time": "...", "event": "..."} ]
   - "legal_sections": [ "e.g. BNS 61", "IPC 302" ]

Ensure valid JSON output. Return ONLY the raw JSON object, without markdown formatting or code blocks."#;

const TEXT_NER_PROMPT: &str = r#"You are an advanced police intelligence and legal analysis AI.
Analyze the following document text and extract all intelligence into a structured JSON object:

Include the following fields:
1. "transcribed_text": The complete document text.
2. "summary": An executive summary of the case facts, allegations, timeline, and key findings.
3. "document_classification": Type of document (e.g., "FIR Copy", "Witness Statement", "Interrogation Report", "Case Diary", "Seizure Report").
4. "entities": Extract all identifiable entities:
   - "persons": [ {"name": "...", "role": "suspect"|"accused"|"victim"|"witness"|"informant"|"police"|"other", "alias": "..."} ]
   - "phone_numbers": [ {"number": "...", "owner": "...", "context": "..."} ]
   - "financial_accounts": [ {"account_number": "...", "bank": "...", "holder": "..."} ]
   - "vehicles": [ {"registration_number": "...", "make_model": "...", "color": "..."} ]
   - "weapons_objects": [ {"item": "...", "description": "..."} ]
   - "locations": [ {"place": "...", "address": "...", "significance": "..."} ]
   - "dates_times": [ {"date": "...", "time": "...", "event": "..."} ]
   - "legal_sections": [ "e.g. BNS 61", "IPC 302" ]
5. "conspiracy_leads": [ {"source_entity": "...", "target_entity": "...", "relationship": "...", "description": "..."} ]

Ensure valid JSON output. Return ONLY the raw JSON object, without markdown formatting or code blocks."#;

const VOICE_ANALYSIS_PROMPT: &str = r#"You are an acoustic and audio intelligence analyst AI.
Analyze the following recorded conversation / transcription and extract all intelligence into a structured JSON object:

Include the following fields:
1. "transcribed_text": The complete conversation transcript with speaker attributions if present.
2. "summary": A concise overview of the conversation, key disclosures, and operational significance.
3. "dialogue_analysis":
   - "speakers": [ {"speaker_id": "...", "identified_person": "...", "tone": "...", "role": "..."} ]
   - "key_statements": [ {"speaker": "...", "statement": "...", "significance": "..."} ]
4. "entities": Extract all identifiable entities mentioned:
   - "persons": [ {"name": "...", "role": "suspect"|"victim"|"witness"|"associate"|"other", "alias": "..."} ]
   - "phone_numbers": [ {"number": "...", "owner": "..."} ]
   - "financial_accounts": [ {"account_number": "...", "bank": "...", "holder": "..."} ]
   - "vehicles": [ {"registration_number": "...", "details": "..."} ]
   - "weapons_objects": [ {"item": "...", "details": "..."} ]
   - "locations": [ {"place": "...", "context": "..."} ]
   - "dates_times": [ {"date": "...", "time": "...", "event": "..."} ]
5. "threat_urgency_level": "low" | "medium" | "high" | "critical"

Ensure valid JSON output. Return ONLY the raw JSON object, without markdown formatting or code blocks."#;

// ---------------------------------------------------------------------------
// Response Deserialization Types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

// ---------------------------------------------------------------------------
// NvidiaKimiProcessor
// ---------------------------------------------------------------------------

/// Universal Document Processor powered by Moonshot AI Kimi-K3 via NVIDIA NIM.
///
/// Handles Image (OCR & visual forensics), Text (in-depth NER & conspiracy graphs),
/// and Voice (audio transcript analysis) documents.
pub struct NvidiaKimiProcessor {
    api_key: String,
    model: String,
    endpoint: String,
    client: reqwest::Client,
}

impl NvidiaKimiProcessor {
    /// Create a new processor with an explicit API key and model name.
    pub fn new(api_key: String, model: String) -> Self {
        let endpoint = std::env::var("NVIDIA_ENDPOINT")
            .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1/chat/completions".to_string());
        Self {
            api_key,
            model,
            endpoint,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(90))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Construct from environment variables:
    /// - `NVIDIA_API_KEY`: Required.
    /// - `NVIDIA_MODEL`: Defaults to `"nvidia/nemotron-3-nano-omni-30b-a3b-reasoning"`.
    /// - `NVIDIA_ENDPOINT`: Defaults to `"https://integrate.api.nvidia.com/v1/chat/completions"`.
    pub fn from_env() -> Result<Self, DocumentErrors> {
        let api_key = std::env::var("NVIDIA_API_KEY")
            .map_err(|_| DocumentErrors::StorageError("NVIDIA_API_KEY is not set".to_string()))?;
        let model = std::env::var("NVIDIA_MODEL")
            .unwrap_or_else(|_| "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning".to_string());
        Ok(Self::new(api_key, model))
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Clean response string and parse into `serde_json::Value`.
    fn parse_model_json(content: &str) -> serde_json::Value {
        let cleaned = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        match serde_json::from_str::<serde_json::Value>(cleaned) {
            Ok(val) => val,
            Err(e) => {
                tracing::warn!(
                    "[NvidiaProcessor] Failed to parse model output as JSON: {}. Storing raw_text.",
                    e
                );
                serde_json::json!({
                    "raw_text": content,
                    "transcribed_text": content,
                    "summary": "Extracted unstructured text from document."
                })
            }
        }
    }
}

pub type NvidiaProcessor = NvidiaKimiProcessor;

#[async_trait]
impl DocumentProcessor for NvidiaKimiProcessor {
    async fn process(
        &self,
        doc: &dyn Document,
        db: &DatabaseConnection,
    ) -> Result<(), DocumentErrors> {
        let doc_id = doc.id();
        tracing::info!("[NvidiaProcessor][START] Processing doc_id={}", doc_id);

        // 1. Fetch document record from DB to determine its exact DocumentType
        let doc_model = document::Entity::find_by_id(doc_id)
            .one(db)
            .await
            .map_err(DocumentErrors::DatabaseError)?
            .ok_or_else(|| {
                DocumentErrors::NotFound(format!("Document with id={} not found", doc_id))
            })?;

        let doc_type = doc_model.r#type;
        tracing::info!(
            "[NvidiaProcessor] doc_id={} has type={:?}, mime='{}'",
            doc_id,
            doc_type,
            doc.mime_type()
        );

        // 2. Fetch raw file bytes from S3/MinIO
        let bytes = doc.fetch().await.map_err(|e| {
            tracing::error!(
                "[NvidiaProcessor][FETCH_ERROR] Failed to fetch bytes for doc_id={}: {}",
                doc_id,
                e
            );
            e
        })?;

        // 3. Build payload according to document type
        let payload = match doc_type {
            DocumentType::Image => {
                let mime = doc.mime_type();
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let data_url = format!("data:{};base64,{}", mime, b64);

                serde_json::json!({
                    "model": self.model,
                    "messages": [
                        {
                            "role": "user",
                            "content": [
                                {
                                    "type": "text",
                                    "text": IMAGE_OCR_PROMPT
                                },
                                {
                                    "type": "image_url",
                                    "image_url": {
                                        "url": data_url
                                    }
                                }
                            ]
                        }
                    ],
                    "temperature": 0.6,
                    "top_p": 0.95,
                    "max_tokens": 16384,
                    "reasoning_budget": 4096
                })
            }
            DocumentType::Text => {
                let text_content = String::from_utf8_lossy(&bytes);
                let prompt = format!(
                    "{}\n\n--- DOCUMENT CONTENT ---\n{}",
                    TEXT_NER_PROMPT, text_content
                );

                serde_json::json!({
                    "model": self.model,
                    "messages": [
                        {
                            "role": "user",
                            "content": prompt
                        }
                    ],
                    "temperature": 0.6,
                    "top_p": 0.95,
                    "max_tokens": 16384,
                    "reasoning_budget": 4096
                })
            }
            DocumentType::Voice => {
                // Check if the uploaded bytes are already a text transcript (e.g. text/vtt/json)
                let text_sample = String::from_utf8(bytes.clone());
                let prompt = match text_sample {
                    Ok(transcript) => format!(
                        "{}\n\n--- AUDIO TRANSCRIPTION ---\n{}",
                        VOICE_ANALYSIS_PROMPT, transcript
                    ),
                    Err(_) => {
                        // Binary audio file (e.g. WAV/MP3) without companion transcript
                        format!(
                            "{}\n\n--- AUDIO RECORDING METADATA ---\nDocument Title: {}\nDescription: {}\n(Note: Binary audio stream uploaded; analyzed metadata and title).",
                            VOICE_ANALYSIS_PROMPT, doc_model.title, doc_model.description
                        )
                    }
                };

                serde_json::json!({
                    "model": self.model,
                    "messages": [
                        {
                            "role": "user",
                            "content": prompt
                        }
                    ],
                    "temperature": 0.6,
                    "top_p": 0.95,
                    "max_tokens": 16384,
                    "reasoning_budget": 4096
                })
            }
        };

        // 4. Dispatch request to NVIDIA NIM
        tracing::info!(
            "[NvidiaProcessor][HTTP_POST] Dispatching request to {} (model: '{}') for doc_id={}",
            self.endpoint,
            self.model,
            doc_id
        );

        let start_time = std::time::Instant::now();
        let mut response = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(
                    "[NvidiaProcessor][NETWORK_ERROR] Request failed for doc_id={}: {}",
                    doc_id,
                    e
                );
                DocumentErrors::StorageError(format!("NVIDIA NIM request failed: {}", e))
            })?;

        // If 503 (worker local total limit reached), wait 3s and retry once
        if response.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            tracing::warn!(
                "[NvidiaProcessor][RETRY] 503 Service Unavailable received for doc_id={}. Retrying in 3s...",
                doc_id
            );
            tokio::time::sleep(Duration::from_secs(3)).await;
            if let Ok(retry_resp) = self
                .client
                .post(&self.endpoint)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await
            {
                response = retry_resp;
            }
        }

        let elapsed = start_time.elapsed();

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(
                "[NvidiaProcessor][API_ERROR] NVIDIA API returned status={} for doc_id={} in {:?}: {}",
                status,
                doc_id,
                elapsed,
                body
            );
            return Err(DocumentErrors::StorageError(format!(
                "NVIDIA API returned HTTP {}: {}",
                status, body
            )));
        }

        tracing::info!(
            "[NvidiaProcessor][SUCCESS] NVIDIA API responded in {:?} for doc_id={}",
            elapsed,
            doc_id
        );

        // 5. Parse response content
        let completion: ChatCompletionResponse = response.json().await.map_err(|e| {
            tracing::error!(
                "[NvidiaProcessor][JSON_PARSE_ERROR] Failed to deserialize NVIDIA response for doc_id={}: {}",
                doc_id,
                e
            );
            DocumentErrors::StorageError(format!("Failed to parse NVIDIA response JSON: {}", e))
        })?;

        let content = completion
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| {
                DocumentErrors::StorageError("No content returned in NVIDIA response choice".into())
            })?;

        let mut extracted = Self::parse_model_json(content);

        // Ensure transcribed_text field exists for downstream consumers
        if let Some(obj) = extracted.as_object_mut() {
            if !obj.contains_key("transcribed_text") {
                obj.insert(
                    "transcribed_text".to_string(),
                    serde_json::Value::String(content.to_string()),
                );
            }
        }

        // 6. Persist extracted JSON atomically into the database
        doc.update_extracted_information(extracted, db).await?;
        tracing::info!(
            "[NvidiaProcessor][FINISH] Successfully persisted extracted information for doc_id={}",
            doc_id
        );

        Ok(())
    }

    /// Background cron function (provided for full compliance with `DocumentProcessor`).
    /// Queries all unprocessed confirmed documents across Image, Text, and Voice types.
    async fn cron_func(
        &self,
        db: &DatabaseConnection,
        store: &S3ObjectStore,
    ) -> Result<(), DocumentErrors> {
        tracing::debug!("[NvidiaProcessor][CRON] Scanning for unprocessed documents...");

        let documents = document::Entity::find()
            .filter(document::Column::Status.eq(DocumentStatus::Success))
            .filter(document::Column::ExtractedInformation.is_null())
            .all(db)
            .await
            .map_err(|e| {
                tracing::error!(
                    "[NvidiaProcessor][DB_ERROR] Failed to query unprocessed documents: {}",
                    e
                );
                DocumentErrors::DatabaseError(e)
            })?;

        if documents.is_empty() {
            return Ok(());
        }

        tracing::info!(
            "[NvidiaProcessor][CRON] Found {} unprocessed document(s)",
            documents.len()
        );

        for model in documents {
            let doc_id = model.id;

            // Transition status → Processing
            let mut active: document::ActiveModel = model.clone().into();
            active.status = Set(DocumentStatus::Processing);
            active.updated_at = Set(chrono::Utc::now().fixed_offset());
            if let Err(e) = active.update(db).await {
                tracing::error!(
                    "[NvidiaProcessor][DB_ERROR] Failed to set status=Processing for doc_id={}: {}",
                    doc_id,
                    e
                );
                continue;
            }

            // Wrap in matching Document implementation
            let process_result = match model.r#type {
                DocumentType::Image => {
                    let image_doc = Image::new(doc_id, model.object_key.clone(), store.clone());
                    self.process(&image_doc, db).await
                }
                DocumentType::Text => {
                    let text_doc = Text::new(doc_id, model.object_key.clone(), store.clone());
                    self.process(&text_doc, db).await
                }
                DocumentType::Voice => {
                    let voice_doc = Voice::new(doc_id, model.object_key.clone(), store.clone());
                    self.process(&voice_doc, db).await
                }
            };

            if let Err(e) = process_result {
                tracing::error!(
                    "[NvidiaProcessor][PROCESSING_ERROR] Error processing doc_id={}: {}",
                    doc_id,
                    e
                );
            }

            // Transition status → Finish
            if let Ok(Some(d)) = document::Entity::find_by_id(doc_id).one(db).await {
                let mut active: document::ActiveModel = d.into();
                active.status = Set(DocumentStatus::Finish);
                active.updated_at = Set(chrono::Utc::now().fixed_offset());
                let _ = active.update(db).await;
                tracing::info!(
                    "[NvidiaProcessor][STATUS_TRANSITION] doc_id={} → Finish",
                    doc_id
                );
            }
        }

        Ok(())
    }
}
