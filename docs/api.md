# Document Management Service - API Documentation

This service provides an API for document storage orchestration, presigned S3 upload/download lifecycle management, and metadata persistence using PostgreSQL and S3-compatible object storage (e.g., AWS S3, MinIO).

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

### `Document` Object Schema
| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `string` (UUID v4) | Unique document identifier. |
| `title` | `string` | Display title for the document. |
| `description` | `string` | Brief description of the document contents. |
| `status` | `string` (`DocumentStatus`) | Current lifecycle state (`pending`, `processing`, `success`, `failed`, `finish`). |
| `object_key` | `string` | Storage path in S3 bucket (Format: `{document_id}/{file_name}`). |
| `extracted_information` | `object` or `null` | JSON payload containing OCR or parsing outputs if available. |
| `case_id` | `string` (UUID v4) | Foreign key identifier of the parent case. |
| `created_at` | `string` (ISO 8601) | Timestamp with timezone of document creation. |
| `updated_at` | `string` (ISO 8601) | Timestamp with timezone of last status/metadata update. |

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

- **Request Example**:
  ```json
  {
    "title": "Police Report",
    "description": "Scanned incident report from the precinct",
    "file_name": "incident_report_2026.pdf",
    "case_id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11"
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
