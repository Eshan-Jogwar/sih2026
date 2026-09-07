# Heterogeneous Graph Transformer (HGT) GNN Architecture & Integration Guide
**SIH 2026 - Document Management & Syndicate Intelligence Platform**

---

## 1. Executive Summary

Organised criminal syndicates deliberately distribute illegal operations across multiple police jurisdictions, distinct FIRs, burner phones, and shell bank accounts. Isolated case-by-case investigations fail to detect systemic patterns.

The **GNN Link Prediction Subsystem** resolves this by integrating a **Heterogeneous Graph Transformer (HGT)** neural model. The system synthesizes evidentiary data across **all cases** in the database to infer hidden criminal conspiracy links, identify syndicate kingpins, trace benami financial conduits, and generate statutory prosecution recommendations (e.g., under Bharatiya Nyaya Sanhita [BNS] Section 61 / IPC Section 120B).

---

## 2. End-to-End Pipeline Architecture

```
+---------------------------------------------------------------------------------------------------------+
|                                        POSTGRESQL DATABASE                                              |
|  Documents Table (`extracted_information` JSON)               `gnn_link_prediction` Table               |
|  Case 1 (FIR 101): [Suspect: Rajesh, Phone: 9871...]          - target_entity_id                        |
|  Case 2 (FIR 204): [Accused: Vikram, Bank: HDFC-5010...]      - candidate_entity_id                     |
|  Case 3 (CDR Log): [Caller: 9871..., Receiver: 9871...]       - link_probability (e.g. 0.824)           |
|                                                               - recommendation: "BNS Sec 61 inquiry..." |
+---------------------------------------------------------------------------------------------------------+
                    ▲                                                                │
                    │ Query docs with                                                │ Save predictions
                    │ extracted_information                                          ▼
+───────────────────┴────────────────────────────────────────────────────────────────┴───────────────────+
|                                    RUST SERVER (AXUM)                                                  |
|                                                                                                         |
|   1. `GnnProcessor` (implements `DocumentProcessor`)                                                    |
|      - `cron_func`: Periodically or manually queries collective documents across ALL cases             |
|      - `build_graph_from_documents`: Constructs heterogeneous GraphPayload                              |
|                                                                                                         |
|   2. Endpoints:                                                                                         |
|      - `POST /api/gnn/trigger`: Triggers collective multi-case inference pipeline                       |
|      - `GET  /api/cases/{case_id}/graph`: Case-scoped Cytoscape.js canvas (evidentiary + GNN links)    |
|      - `GET  /api/gnn/graph`: Global multi-case syndicate Cytoscape.js canvas                           |
+──────────────────────────────────────────────────┬──────────────────────────────────────────────────────+
                                                   │
                                                   │ POST /api/v1/graph/predict-conspiracy
                                                   ▼
+---------------------------------------------------------------------------------------------------------+
|                               PYTHON HGT GNN MICROSERVICE (PyG)                                         |
|                                                                                                         |
|   - Architecture: Heterogeneous Graph Transformer (HGT)                                                 |
|   - Node Types: `person` (32d), `phone` (3d), `financial_account` (8d), `object` (17d), `phantom` (16d) |
|   - Relational Message Passing: Cross-relation attention mechanism                                      |
|   - Output: Predicted conspirator entities, probability scores (0.0 - 1.0), and legal recommendations   |
+---------------------------------------------------------------------------------------------------------+
```

---

## 3. Core Design Principles

### 3.1 Collective Multi-Case Inference (No Per-Case Isolation)
- In standard document processing, an OCR parser or vision model operates on an individual document or case.
- In contrast, criminal conspiracy inference **MUST span across cases**.
- The `gnn_link_prediction` table **contains no `case_id` column and no foreign key constraints**. Predicted links connect entities globally (e.g. connecting a suspect in Case A to a mule account in Case B).

### 3.2 Ephemeral Vector Embeddings
- Vector embeddings computed during neural message passing are kept in-memory inside the microservice and are not persisted to disk.
- Only the actionable analytical outputs—predicted edge types, link probabilities, hypothesis flags, and statutory recommendations—are stored in the relational database.

### 3.3 Strict Adherence to the `DocumentProcessor` Trait
- `GnnProcessor` implements the `DocumentProcessor` trait located in `libs/document/src/processors/base.rs`.
- Collective inference is executed within `cron_func(&self, db, storage)`, allowing it to be triggered periodically via background tasks or on-demand via the API router.

---

## 4. Heterogeneous Graph Schema & Feature Vectors

The graph payload sent to the HGT microservice matches the following exact feature dimensions:

| Node Type | Feature Dimension | Extracted Attributes & Semantic Representation |
| :--- | :--- | :--- |
| `person` | **32** | Deterministic name hash embedding, degree centrality, role indicators (`Suspect`, `Accused`, `Complainant`), mention frequency across cases. |
| `phone` | **3** | Normalized 10-digit number hash, CDR call frequency, unique contacts ratio. |
| `financial_account` | **8** | Bank IFSC hash, transaction frequency, inflow/outflow balance ratios, velocity score. |
| `object` | **17** | Vehicle registration / weapon serial hash, seized quantity vector, IMEI identifier. |
| `phantom_entity` | **16** | Shell enterprise identifier, unverified entity score, benami flag indicators. |

### Edge Relations
- `called`: Connects `person` $\rightarrow$ `phone` or `phone` $\rightarrow$ `phone`.
- `transacted_with`: Connects `person` $\rightarrow$ `financial_account` or `financial_account` $\rightarrow$ `financial_account`.
- `associated_with`: Connects `person` $\rightarrow$ `object` or `person` $\rightarrow$ `phantom_entity`.
- `co_accused`: Direct evidentiary connection between two `person` nodes named in the same FIR/complaint.

---

## 5. Database Schema: `gnn_link_prediction`

Generated via SeaORM migration `migration/src/m20260907_160048_gnn.rs`:

```sql
CREATE TABLE gnn_link_prediction (
    id UUID PRIMARY KEY,
    target_entity_id VARCHAR NOT NULL,
    target_entity_type VARCHAR NOT NULL,
    candidate_entity_id VARCHAR NOT NULL,
    candidate_entity_type VARCHAR NOT NULL,
    predicted_edge_type VARCHAR NOT NULL,
    link_probability DOUBLE PRECISION NOT NULL,
    is_hypothesis_flagged BOOLEAN NOT NULL DEFAULT FALSE,
    confidence_threshold DOUBLE PRECISION,
    recommendation TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_gnn_pred_target ON gnn_link_prediction(target_entity_id);
CREATE INDEX idx_gnn_pred_candidate ON gnn_link_prediction(candidate_entity_id);
CREATE INDEX idx_gnn_pred_hypothesis ON gnn_link_prediction(is_hypothesis_flagged);
```

---

## 6. Microservice Contract: `predict-conspiracy`

### Request Payload: `POST /api/v1/graph/predict-conspiracy`

```json
{
  "case_id": "collective_investigation",
  "target_entity_id": "ENT-PERSON-RAJESH_SHARMA",
  "target_entity_type": "person",
  "candidate_entity_ids": [
    "ENT-PERSON-VIKRAM_MALHOTRA",
    "ENT-PERSON-SUNIL_KAPOOR"
  ],
  "confidence_threshold": 0.10,
  "graph": {
    "nodes": {
      "person": {
        "ids": ["ENT-PERSON-RAJESH_SHARMA", "ENT-PERSON-VIKRAM_MALHOTRA"],
        "features": [
          [0.12, 0.45, ... 32 dimensions ...],
          [0.34, 0.11, ... 32 dimensions ...]
        ]
      },
      "phone": {
        "ids": ["ENT-PHONE-9871987654"],
        "features": [[0.98, 0.12, 0.45]]
      },
      "financial_account": {
        "ids": ["ENT-ACC-HDFC-501004128912"],
        "features": [[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]]
      },
      "object": { "ids": [], "features": [] },
      "phantom_entity": { "ids": [], "features": [] }
    },
    "edges": [
      {
        "edge_type": "called",
        "src_indices": [0],
        "dst_indices": [0]
      }
    ]
  }
}
```

### Response Payload

```json
{
  "status": "success",
  "target_entity_id": "ENT-PERSON-RAJESH_SHARMA",
  "predicted_conspirators": [
    {
      "entity_id": "ENT-PERSON-VIKRAM_MALHOTRA",
      "link_probability": 0.824,
      "relationship_type": "co_conspirator",
      "recommendation": "Recommend immediate interrogation under BNS Section 61 (Criminal Conspiracy). Subject acts as high-frequency financial routing intermediary."
    }
  ],
  "latency_ms": 42.5
}
```

---

## 7. API Endpoints Reference

### 7.1 Manual Trigger
- **Route**: `POST /api/gnn/trigger`
- **Description**: Triggers collective graph construction and sends prediction requests to the microservice. Updates `gnn_link_prediction` table.
- **Response**:
  ```json
  { "message": "GNN collective inference triggered and updated successfully" }
  ```

### 7.2 Case-Scoped Cytoscape Graph
- **Route**: `GET /api/cases/{case_id}/graph`
- **Description**: Compiles all evidentiary nodes and edges from documents belonging to `{case_id}` and augments them with cross-case GNN hypothesis predictions from `gnn_link_prediction`.
- **Visual Styles**:
  - `evidentiary`: Solid line (`#64748b`, width `2px`), grounded in documents.
  - `hypothesis`: Dashed line (`#ef4444` if probability $\ge 0.70$, `#f59e0b` otherwise, width `3px`).

### 7.3 Global Multi-Case Syndicate Graph
- **Route**: `GET /api/gnn/graph`
- **Description**: Returns all entities, evidentiary links, and predicted conspiracy ties across all cases in the database. Essential for central intelligence oversight.

---

## 8. Configuration & Environment Variables

All GNN settings are configured in `.env`:

| Variable | Default Value | Description |
| :--- | :--- | :--- |
| `GNN_SERVICE_URL` | `http://127.0.0.1:8001` | Base URL of the Python HGT microservice. |
| `ENABLE_DOCUMENT_PROCESSOR_CRON` | `true` | Boolean flag enabling/disabling the background document processing cron. |
| `DOCUMENT_CRON_INTERVAL_SECS` | `30` | Interval in seconds between background cron executions. |
| `RUST_LOG` | `info,server=debug,document=debug,tower_http=info` | Tracing filter controlling logging verbosity. |

---

## 9. Verification & Testing

Unit tests for the GNN graph builder and feature extraction are located in `libs/document/src/processors/gnn.rs`:

```bash
# Run GNN processor unit test
cargo test processors::gnn

# Run workspace compilation check
cargo check --workspace
```
