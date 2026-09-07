# Document Management Service - API Documentation

This service provides an API for document storage orchestration, presigned S3 upload/download lifecycle management, and metadata persistence using PostgreSQL and S3-compatible object storage (e.g., Supabase Storage, AWS S3, MinIO).

---

## 1. Overview & Architecture

### Server Configuration
- **Host**: Configured via `SERVER_HOST` (default: `0.0.0.0`)
- **Port**: Configured via `SERVER_PORT` (default: `8000`)
- **Base URL**: `http://localhost:8000`
- **Content-Type**: `application/json` (for request and response bodies unless specified otherwise)
- **CORS**: Permissive (`*` origin, headers, and methods) for API consumers

### Upload Flow Sequence

```
+----------+             +-------------+             +------------+             +----------+
| Frontend |             | Axum Server |             | PostgreSQL |             | S3/MinIO |
+----------+             +-------------+             +------------+             +----------+
     |                          |                          |                          |
     | 1. Initiate Upload       |                          |                          |
     |------------------------->|                          |                          |
     |                          | 2. Insert Record         |                          |
     |                          |    (status: 'pending')   |                          |
     |                          |------------------------->|                          |
     |                          |                          |                          |
     |                          | 3. Generate Presigned    |                          |
     |                          |    PUT URL (300s expiry) |                          |
     |                          |---------------------------------------------------->|
     |                          |<----------------------------------------------------|
     |                          |                          |                          |
     | 4. Return document_id    |                          |                          |
     |    & upload_url          |                          |                          |
     |<-------------------------|                          |                          |
     |                                                                                |
     | 5. Direct Binary Upload (HTTP PUT to presigned upload_url)                     |
     |------------------------------------------------------------------------------->|
     |<-------------------------------------------------------------------------------|
     |                                                                                |
     | 6. Confirm Upload                                   |                          |
     |    (success: true/false) |                          |                          |
     |------------------------->|                          |                          |
     |                          | 7. Update Record Status  |                          |
     |                          |    ('success'/'failed')  |                          |
     |                          |------------------------->|                          |
     |                          |                          |                          |
     | 8. Return Updated Doc    |                          |                          |
     |<-------------------------|                          |                          |
```

---

## 2. Common Data Types & Enums

### `DocumentStatus`
Represents the current state of a document in the system:
- `"pending"`: Initial state upon upload initiation. S3 object upload is in progress or pending.
- `"processing"`: Document is currently undergoing background processing or OCR extraction.
- `"success"`: Document has been successfully uploaded to S3 and confirmed.
- `"failed"`: Upload failed, cancelled by client, or failed validation.
- `"finish"`: Terminal state after post-upload processing completes.

### `DocumentType`
Categorises the kind of content a document contains:
- `"image"`: Image-based document (scanned pages, photographs, etc.).
- `"text"`: Text-based document (typed reports, transcripts, etc.).
- `"voice"`: Audio/voice recording.

### `Document` Object Schema
| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `string` (UUID v4) | Unique document identifier. |
| `title` | `string` | Display title for the document. |
| `description` | `string` | Brief description of the document contents. |
| `status` | `string` (`DocumentStatus`) | Current lifecycle state (`pending`, `processing`, `success`, `failed`, `finish`). |
| `document_type` | `string` (`DocumentType`) | Content category of the document (`image`, `text`, `voice`). |
| `object_key` | `string` | Storage path in S3 bucket (Format: `{document_id}/{file_name}`). |
| `extracted_information` | `object` or `null` | JSON payload containing OCR or parsing outputs if available. |
| `case_id` | `string` (UUID v4) | Foreign key identifier of the parent case. |
| `created_at` | `string` (ISO 8601) | Timestamp with timezone of document creation. |
| `updated_at` | `string` (ISO 8601) | Timestamp with timezone of last status/metadata update. |

### `Case` Object Schema
| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `string` (UUID v4) | Unique case identifier. |
| `name` | `string` | Name/title of the case. |
| `created_at` | `string` (ISO 8601) | Timestamp with timezone of case creation. |
| `updated_at` | `string` (ISO 8601) | Timestamp with timezone of last case update. |

### `GnnLinkPrediction` Object Schema
Represents a predicted hidden criminal conspiracy or syndicate link inferred by the Heterogeneous Graph Transformer (HGT) across collective multi-case data:
| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `string` (UUID v4) | Unique link prediction identifier. |
| `target_entity_id` | `string` | Primary subject/target entity ID (e.g. `ENT-PERSON-RAJESH_SHARMA`). |
| `target_entity_type` | `string` | Entity type of the target (e.g. `person`). |
| `candidate_entity_id` | `string` | Inferred accomplice/candidate associate ID. |
| `candidate_entity_type`| `string` | Entity type of the candidate associate (e.g. `person`). |
| `predicted_edge_type` | `string` | Inferred relationship type (e.g. `co_conspirator`, `shell_handler`). |
| `link_probability` | `number` (f64, 0.0–1.0) | Neural model link probability confidence. |
| `is_hypothesis_flagged` | `boolean` | `true` if `link_probability >= confidence_threshold`. |
| `confidence_threshold` | `number` (f64) or `null` | Threshold applied during inference (default `0.10`). |
| `recommendation` | `string` | Statutory/procedural legal recommendation (e.g. BNS Section 61). |
| `created_at` | `string` (ISO 8601) | Timestamp with timezone when hypothesis was generated. |
| `updated_at` | `string` (ISO 8601) | Timestamp with timezone when hypothesis was last updated. |

*Note: The `gnn_link_prediction` table operates globally across all cases and does not contain a `case_id` foreign key.*

### `CytoscapeGraphResponse` Schema (Graph Canvas DTO)
Format ready for direct ingestion by frontend Cytoscape.js canvases:
| Field | Type | Description |
| :--- | :--- | :--- |
| `status` | `string` | Execution status (`"success"`). |
| `case_id` | `string` or `null` | Case name (for case-scoped graph) or `null` (for global graph). |
| `meta` | `GraphMeta` | Metadata counters (`total_nodes`, `total_edges`, `evidentiary_edges`, `predicted_edges`, `last_analyzed`). |
| `elements` | `CytoscapeElements` | Graph data containing `nodes` and `edges` arrays. |

#### `CytoscapeNode.data`
| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `string` | Unique entity node ID (e.g. `ENT-PERSON-RAJESH_SHARMA`, `ENT-PHONE-9871987654`). |
| `label` | `string` | Human-readable node label. |
| `type` | `string` | Category (`person`, `phone`, `financial_account`, `object`, `phantom_entity`). |
| `badge` | `string` (optional) | Visual tag (e.g. `Suspect`, `Candidate Associate`, `Device`). |
| `risk_score` | `number` (optional) | Computed risk or conspiracy probability (0.0–1.0). |
| `attributes` | `object` (optional) | Entity attributes extracted from documents. |

#### `CytoscapeEdge.data`
| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `string` | Unique edge ID (e.g. `edge-case-1`, `edge-gnn-pred-...`). |
| `source` | `string` | Source entity node ID. |
| `target` | `string` | Target entity node ID. |
| `label` | `string` | Edge description (e.g. `called (CDR)`, `co_conspirator (82.4%)`). |
| `category` | `string` | `"evidentiary"` (solid documented evidence) or `"hypothesis"` (dashed GNN prediction). |
| `is_hypothesis` | `boolean` | `false` for evidentiary edges; `true` for GNN predictions. |
| `style` | `string` | `"solid"` for evidentiary; `"dashed"` for hypothesis. |
| `color` | `string` | Hex color code (`#64748b` for evidentiary; `#ef4444` high alert, `#f59e0b` moderate alert). |
| `probability` | `number` (optional) | Link probability (present on hypothesis edges). |
| `recommendation` | `string` (optional) | Legal/procedural recommendation (present on hypothesis edges). |
| `source_document` | `string` (optional) | Title of the source evidentiary document if applicable. |

---

## 3. Endpoints

### 3.1 Health Check

#### `GET /health` or `GET /api/health`
Checks whether the HTTP server is responsive and running.

- **Request**:
  - Headers: None required
  - Body: None

- **Response**:
  - Status: `200 OK`
  - Content-Type: `text/plain`
  - Body: `ok`

- **Example**:
  ```bash
  curl -X GET http://localhost:8000/health
  ```

---

### 3.2 Initiate Document Upload

#### `POST /api/documents/upload/initiate`
Initiates a new document upload session. Creates a record in the database with status set to `pending` and returns an S3 presigned PUT URL valid for 300 seconds (5 minutes).

- **Headers**:
  - `Content-Type: application/json`

- **Request Body**:
  | Field | Type | Required | Description |
  | :--- | :--- | :--- | :--- |
  | `title` | `string` | Yes | Human-readable title for the document. |
  | `description` | `string` | Yes | Brief description of the document. |
  | `file_name` | `string` | Yes | Original file name including extension (e.g., `contract.pdf`). |
  | `case_id` | `string` (UUID) | Yes | Existing case ID this document belongs to. |
  | `document_type` | `string` (`DocumentType`) | Yes | Content category: `"image"`, `"text"`, or `"voice"`. |

- **Request Example**:
  ```json
  {
    "title": "Police Report",
    "description": "Scanned incident report from the precinct",
    "file_name": "incident_report_2026.pdf",
    "case_id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
    "document_type": "image"
  }
  ```

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "document_id": "550e8400-e29b-41d4-a716-446655440000",
      "upload_url": "http://localhost:9000/bucket/550e8400-e29b-41d4-a716-446655440000/incident_report_2026.pdf?X-Amz-Algorithm=...",
      "object_key": "550e8400-e29b-41d4-a716-446655440000/incident_report_2026.pdf"
    }
    ```

- **Client Action Required**:
  The client frontend must now upload the raw file bytes directly to `upload_url` via HTTP `PUT`:
  ```bash
  curl -X PUT -T "incident_report_2026.pdf" "<upload_url>"
  ```

- **Error Responses**:
  - `500 Internal Server Error`: Database insertion failure or S3 signature error.
    ```json
    { "error": "Database error: ..." }
    ```

---

### 3.3 Confirm Document Upload

#### `POST /api/documents/upload/confirm`
Called by the client application after uploading the binary file directly to S3. Updates the document status from `pending` to either `success` or `failed`.

- **Headers**:
  - `Content-Type: application/json`

- **Request Body**:
  | Field | Type | Required | Description |
  | :--- | :--- | :--- | :--- |
  | `document_id` | `string` (UUID) | Yes | The `document_id` returned during the initiate step. |
  | `success` | `boolean` | Yes | `true` if S3 PUT succeeded; `false` if upload failed or timed out. |

- **Request Example**:
  ```json
  {
    "document_id": "550e8400-e29b-41d4-a716-446655440000",
    "success": true
  }
  ```

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "Police Report",
      "description": "Scanned incident report from the precinct",
      "status": "success",
      "document_type": "image",
      "object_key": "550e8400-e29b-41d4-a716-446655440000/incident_report_2026.pdf",
      "extracted_information": null,
      "case_id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
      "created_at": "2026-09-03T19:30:00+05:30",
      "updated_at": "2026-09-03T19:30:15+05:30"
    }
    ```

- **Error Responses**:
  - `404 Not Found`: Document ID does not exist.
    ```json
    { "error": "Document 550e8400-e29b-41d4-a716-446655440000 not found" }
    ```
  - `400 Bad Request`: Document is not in `pending` state (cannot confirm an already confirmed or finalized document).
    ```json
    { "error": "Document 550e8400-e29b-41d4-a716-446655440000 is not in Pending status (current: Success)" }
    ```

---

### 3.4 Get Presigned Download URL

#### `GET /api/documents/{id}/download`
Generates a temporary presigned GET URL to securely stream or download the document directly from S3 without passing bytes through the server. The URL is valid for 300 seconds (5 minutes).

- **Path Parameters**:
  - `id` (`string`, UUID): The ID of the document to download.

- **Request Body**: None

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "download_url": "http://localhost:9000/bucket/550e8400-e29b-41d4-a716-446655440000/incident_report_2026.pdf?X-Amz-Algorithm=..."
    }
    ```

- **Error Responses**:
  - `404 Not Found`: Document does not exist.
    ```json
    { "error": "Document 550e8400-e29b-41d4-a716-446655440000 not found" }
    ```
  - `500 Internal Server Error`: S3 signing error.

---

### 3.5 Get Document Metadata

#### `GET /api/documents/{id}`
Retrieves document metadata, current status, S3 object key, and extracted information.

- **Path Parameters**:
  - `id` (`string`, UUID): The ID of the document.

- **Request Body**: None

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "Police Report",
      "description": "Scanned incident report from the precinct",
      "status": "success",
      "document_type": "image",
      "object_key": "550e8400-e29b-41d4-a716-446655440000/incident_report_2026.pdf",
      "extracted_information": null,
      "case_id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
      "created_at": "2026-09-03T19:30:00+05:30",
      "updated_at": "2026-09-03T19:30:15+05:30"
    }
    ```

- **Error Responses**:
  - `404 Not Found`:
    ```json
    { "error": "Document 550e8400-e29b-41d4-a716-446655440000 not found" }
    ```

---

### 3.6 List Documents

#### `GET /api/documents`
Lists documents from the database. Optionally filters by `case_id`.

- **Query Parameters**:
  | Parameter | Type | Required | Description |
  | :--- | :--- | :--- | :--- |
  | `case_id` | `string` (UUID) | No | Filter documents associated with a specific case. |

- **Request Examples**:
  - Fetch all documents: `GET /api/documents`
  - Filter by case: `GET /api/documents?case_id=a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11`

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    [
      {
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "title": "Police Report",
        "description": "Scanned incident report from the precinct",
        "status": "success",
        "document_type": "image",
        "object_key": "550e8400-e29b-41d4-a716-446655440000/incident_report_2026.pdf",
        "extracted_information": null,
        "case_id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
        "created_at": "2026-09-03T19:30:00+05:30",
        "updated_at": "2026-09-03T19:30:15+05:30"
      }
    ]
    ```

---

### 3.7 Delete Document

#### `DELETE /api/documents/{id}`
Deletes the document from the S3 object store first, followed by deleting the record from PostgreSQL.

- **Path Parameters**:
  - `id` (`string`, UUID): The ID of the document to delete.

- **Request Body**: None

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "message": "Document 550e8400-e29b-41d4-a716-446655440000 deleted"
    }
    ```

- **Error Responses**:
  - `404 Not Found`: Document ID does not exist in the database.
    ```json
    { "error": "Document 550e8400-e29b-41d4-a716-446655440000 not found" }
    ```
  - `500 Internal Server Error`: Failed to delete object from S3 or failed database deletion query.
    ```json
    { "error": "Storage error: S3 DELETE failed with status 500" }
    ```

---

### 3.8 Create Case

#### `POST /api/cases`
Creates a new case in the database.

- **Headers**:
  - `Content-Type: application/json`

- **Request Body**:
  | Field | Type | Required | Description |
  | :--- | :--- | :--- | :--- |
  | `name` | `string` | Yes | Name or title of the case (non-empty). |

- **Request Example**:
  ```json
  {
    "name": "Financial Fraud Investigation"
  }
  ```

- **Success Response**:
  - Status: `201 Created`
  - Body:
    ```json
    {
      "id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
      "name": "Financial Fraud Investigation",
      "created_at": "2026-09-03T19:30:00+05:30",
      "updated_at": "2026-09-03T19:30:00+05:30"
    }
    ```

- **Error Responses**:
  - `400 Bad Request`: Case name is empty.
    ```json
    { "error": "Case name cannot be empty" }
    ```
  - `500 Internal Server Error`: Database insertion failure.

---

### 3.9 List Cases

#### `GET /api/cases`
Retrieves a list of all cases in the database.

- **Request**:
  - Headers: None required
  - Body: None

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    [
      {
        "id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
        "name": "Financial Fraud Investigation",
        "created_at": "2026-09-03T19:30:00+05:30",
        "updated_at": "2026-09-03T19:30:00+05:30"
      }
    ]
    ```

---

### 3.10 Get Case Details

#### `GET /api/cases/{id}`
Retrieves metadata for a specific case by its ID.

- **Path Parameters**:
  - `id` (`string`, UUID): Unique identifier of the case.

- **Request Body**: None

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
      "name": "Financial Fraud Investigation",
      "created_at": "2026-09-03T19:30:00+05:30",
      "updated_at": "2026-09-03T19:30:00+05:30"
    }
    ```

- **Error Responses**:
  - `404 Not Found`:
    ```json
    { "error": "Case a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11 not found" }
    ```

---

### 3.11 Update Case

#### `PUT /api/cases/{id}`
Updates a case's name.

- **Path Parameters**:
  - `id` (`string`, UUID): Unique identifier of the case.

- **Headers**:
  - `Content-Type: application/json`

- **Request Body**:
  | Field | Type | Required | Description |
  | :--- | :--- | :--- | :--- |
  | `name` | `string` | Yes | New name or title for the case. |

- **Request Example**:
  ```json
  {
    "name": "Financial Fraud Investigation - Closed"
  }
  ```

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
      "name": "Financial Fraud Investigation - Closed",
      "created_at": "2026-09-03T19:30:00+05:30",
      "updated_at": "2026-09-03T19:35:10+05:30"
    }
    ```

- **Error Responses**:
  - `400 Bad Request`: Empty name.
  - `404 Not Found`: Case does not exist.

---

### 3.12 Delete Case

#### `DELETE /api/cases/{id}`
Deletes a case by its ID. PostgreSQL schema's `ON DELETE CASCADE` removes all associated documents.

- **Path Parameters**:
  - `id` (`string`, UUID): Unique identifier of the case.

- **Request Body**: None

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "message": "Case a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11 deleted"
    }
    ```

- **Error Responses**:
  - `404 Not Found`: Case not found.
  - `500 Internal Server Error`: Database deletion error.

---

### 3.13 List Documents for a Case

#### `GET /api/cases/{id}/documents`
Convenience route to list all documents associated with a specific case. (Equivalent to `GET /api/documents?case_id={id}`).

- **Path Parameters**:
  - `id` (`string`, UUID): Unique identifier of the case.

- **Success Response**:
  - Status: `200 OK`
  - Body: Array of `Document` objects (see `Document` schema).

---

### 3.14 Trigger GNN Collective Inference

#### `POST /api/gnn/trigger`
Triggers the collective multi-case GNN pipeline (`GnnProcessor::cron_func`). The server queries all documents across all cases where `extracted_information IS NOT NULL`, constructs the collective heterogeneous graph matching HGT model specifications, sends it to `POST {GNN_SERVICE_URL}/api/v1/graph/predict-conspiracy`, and persists inferred hypothesis links into the `gnn_link_prediction` database table.

- **Headers**: None required
- **Request Body**: None

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "message": "GNN collective inference triggered and updated successfully"
    }
    ```

- **Error Responses**:
  - `500 Internal Server Error`: Database error or microservice failure.
  - `502 Bad Gateway`: Upstream network failure connecting to `GNN_SERVICE_URL`.

---

### 3.15 Get Case Graph (Cytoscape.js Canvas)

#### `GET /api/cases/{case_id}/graph`
Fetches a unified evidentiary and hypothesis graph for a specific case, formatted for direct consumption by Cytoscape.js. Evidentiary relationships (e.g., CDR call records, bank transfers, co-accused FIR mentions) extracted from case documents are styled as solid lines (`#64748b`), while cross-case GNN predicted criminal conspiracy hypotheses are overlaid as dashed lines (`#ef4444` for high alert $\ge 70\%$, `#f59e0b` for moderate alert).

- **Path Parameters**:
  - `case_id` (`string`, UUID): Unique identifier of the case.

- **Success Response**:
  - Status: `200 OK`
  - Body:
    ```json
    {
      "status": "success",
      "case_id": "FIR 101/2026",
      "meta": {
        "total_nodes": 8,
        "total_edges": 9,
        "evidentiary_edges": 6,
        "predicted_edges": 3,
        "last_analyzed": "2026-09-07T22:30:00+05:30"
      },
      "elements": {
        "nodes": [
          {
            "data": {
              "id": "ENT-PERSON-RAJESH_SHARMA",
              "label": "RAJESH SHARMA",
              "type": "person",
              "badge": "Suspect",
              "risk_score": 0.85,
              "attributes": {
                "source": "FIR 101"
              }
            }
          },
          {
            "data": {
              "id": "ENT-PERSON-VIKRAM_MALHOTRA",
              "label": "VIKRAM MALHOTRA",
              "type": "person",
              "badge": "Candidate Associate",
              "risk_score": 0.824,
              "attributes": {}
            }
          }
        ],
        "edges": [
          {
            "data": {
              "id": "edge-evidentiary-1",
              "source": "ENT-PERSON-RAJESH_SHARMA",
              "target": "ENT-PHONE-9871987654",
              "label": "used_device (FIR 101)",
              "category": "evidentiary",
              "is_hypothesis": false,
              "style": "solid",
              "color": "#64748b",
              "source_document": "FIR 101"
            }
          },
          {
            "data": {
              "id": "edge-gnn-pred-8f4b52b2-601e-4c74-a021-f09c62394392",
              "source": "ENT-PERSON-RAJESH_SHARMA",
              "target": "ENT-PERSON-VIKRAM_MALHOTRA",
              "label": "co_conspirator (82.4%)",
              "category": "hypothesis",
              "is_hypothesis": true,
              "style": "dashed",
              "color": "#ef4444",
              "probability": 0.824,
              "recommendation": "Recommend immediate interrogation under BNS Section 61 (Criminal Conspiracy). Subject acts as high-frequency intermediary.",
              "source_document": null
            }
          }
        ]
      }
    }
    ```

- **Error Responses**:
  - `404 Not Found`: Case does not exist.
  - `500 Internal Server Error`: Database query error.

---

### 3.16 Get Global Multi-Case Graph (Cytoscape.js Canvas)

#### `GET /api/gnn/graph`
Returns the global multi-case graph across all cases and documents in the entire system. It visualizes systemic criminal syndicates, shared phone numbers, recurring bank accounts, and cross-case criminal conspiracy hypotheses.

- **Headers**: None required
- **Request Body**: None

- **Success Response**:
  - Status: `200 OK`
  - Body: Same schema as `GET /api/cases/{case_id}/graph`, with `case_id: null`.

- **Error Responses**:
  - `500 Internal Server Error`: Database query error.

---

## 4. Error Handling Summary

All API errors return a standard JSON payload with a descriptive error message:

```json
{
  "error": "Detailed error message here"
}
```

| HTTP Status Code | Condition | Cause |
| :--- | :--- | :--- |
| `400 Bad Request` | `ValidationError` | Invalid state transition (e.g. confirming a non-pending document) or malformed payload. |
| `404 Not Found` | `NotFound` | Document ID does not exist. |
| `500 Internal Server Error` | `StorageError` or `DatabaseError` | S3 network/permission failures, PostgreSQL query failures. |
| `502 Bad Gateway` | `NetworkError` | Upstream connection failure while communicating with S3. |
