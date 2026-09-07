# Frontend UI/UX Design & Architecture Specification
**Document Management & Investigation Intelligence System (SIH 2026)**
*Target Audience: Frontend Engineers, Product Designers, QA Engineers*

---

## 1. Executive Summary & Design Vision

The Document Management Service is an evidentiary and investigative document orchestration platform designed to handle multi-modal case assets—including scanned FIRs/police reports, printed & typed statements, and audio recordings. 

### Key UX Challenges to Solve
1. **Zero-Latency Ingestion**: Investigators upload large files (high-res scans, audio recordings). The UI must handle client-direct S3 presigned PUT uploads seamlessly without stalling the user workflow.
2. **Evidentiary Integrity & Status Clarity**: Clear visual tracking across the 5 document lifecycle states (`pending`, `processing`, `success`, `failed`, `finish`).
3. **Multi-Modal Inspection**: Side-by-side inspection for raw document media (images, audio waveforms, text) alongside AI/OCR `extracted_information`.
4. **Hierarchical Case Scoping**: Intuitive case-driven navigation with quick filtering across modalities (`image`, `text`, `voice`).

---

## 2. Information Architecture & Navigation

```
[App Shell]
 ├── Top Navigation Bar (Global Search, Active Case Selector, Quick Upload Action `⌘+U`, Health Indicator)
 └── Main Workspace
      ├── Route: `/cases` (Case Directory & Aggregate Analytics)
      ├── Route: `/cases/:caseId` (Case Workspace: Evidentiary Feed, Filter Matrix, File Ingestion Zone)
      ├── Route: `/cases/:caseId/graph` (Case Knowledge Graph & GNN Conspiracy Inspector)
      ├── Route: `/cases/:caseId/documents/:docId` (Deep Inspector: Split View Viewer + OCR / Metadata Drawer)
      └── Route: `/syndicate-graph` (Global Multi-Case Crime Syndicate Knowledge Graph)
```

### Route Map & View Hierarchy
| Route | Primary Purpose | Key Components |
| :--- | :--- | :--- |
| `/cases` | Case portfolio overview | `CaseGrid`, `NewCaseModal`, `CaseStatsWidget`, `RecentDocsFeed` |
| `/cases/:caseId` | Primary investigation workspace | `CaseHeader`, `DocumentFilterBar`, `DocumentTable` / `DocumentGrid`, `DirectUploadDropzone` |
| `/cases/:caseId/graph` | Case evidentiary graph + GNN predictions | `CytoscapeCanvas`, `GraphFilterToolbar`, `HypothesisDrawer`, `TriggerGnnButton` |
| `/cases/:caseId/documents/:docId` | Evidentiary verification & OCR analysis | `MediaViewer` (Zoomable Image / Audio Player / Text), `ExtractedInfoDrawer`, `MetadataPanel`, `DownloadAction` |
| `/syndicate-graph` | Global cross-case syndicate analysis | `GlobalCytoscapeCanvas`, `ClusterFilter`, `CrossCaseHypothesisTable`, `MetricsRibbon` |

---

## 3. Design System & Theming Tokens

The frontend should implement an adaptive, high-contrast, clean aesthetic (Dark-first with full Light mode support) designed for focus and low visual fatigue during prolonged investigative work.

### Color Palette (Tailwind Semantic Tokens)
- **Background Surfaces**:
  - Primary App Background: `bg-slate-950` (dark) / `bg-slate-50` (light)
  - Card & Container Surface: `bg-slate-900` (dark) / `bg-white` (light)
  - Interactive Hover / Sub-surfaces: `bg-slate-800/60` (dark) / `bg-slate-100` (light)
  - Borders: `border-slate-800` (dark) / `border-slate-200` (light)
- **Primary Accents**:
  - Indigo/Electric Blue: `text-indigo-400`, `bg-indigo-600 hover:bg-indigo-500` (action items, highlights, primary buttons)
- **Status Badges & Tokens**:
  - `pending`: Amber/Orange (`bg-amber-500/10 text-amber-400 border-amber-500/30`) — Uploading or awaiting S3 PUT
  - `processing`: Blue/Indigo with animated pulse (`bg-blue-500/10 text-blue-400 border-blue-500/30`) — OCR/Extraction pipeline active
  - `success`: Teal/Cyan (`bg-teal-500/10 text-teal-400 border-teal-500/30`) — S3 confirmed, ready for parsing
  - `finish`: Emerald/Green (`bg-emerald-500/10 text-emerald-400 border-emerald-500/30`) — Fully processed with extracted intelligence
  - `failed`: Rose/Crimson (`bg-rose-500/10 text-rose-400 border-rose-500/30`) — Upload failed, timed out, or rejected
- **Modality Badges**:
  - `image`: Cyan/Sky (`Image` icon, `bg-sky-500/10 text-sky-400`)
  - `text`: Purple (`FileText` icon, `bg-purple-500/10 text-purple-400`)
  - `voice`: Amber/Yellow (`Mic` / `AudioWaveform` icon, `bg-amber-500/10 text-amber-400`)
- **Graph Canvas Node & Edge Theming Tokens**:
  - **Node Palette (by Entity Type)**:
    - `person`: Sky Blue (`#38bdf8`, fill: `#0f172a`, border: `#38bdf8`)
    - `phone`: Emerald Green (`#34d399`, fill: `#064e3b`, border: `#34d399`)
    - `financial_account`: Amber Gold (`#fbbf24`, fill: `#78350f`, border: `#fbbf24`)
    - `object` / Vehicle: Purple (`#c084fc`, fill: `#581c87`, border: `#c084fc`)
    - `phantom_entity`: Rose/Crimson (`#f43f5e`, fill: `#881337`, border: `#f43f5e`)
  - **Edge Palette (Evidentiary vs GNN Hypothesis)**:
    - Evidentiary Edge: Solid Slate (`#64748b`, line-style: `solid`, width: `2px`) — Hard evidence grounded in uploaded case documents/CDRs.
    - High-Alert GNN Hypothesis Edge: Bright Red Dashed (`#ef4444`, line-style: `dashed`, line-dash-pattern: `[6, 3]`, width: `3px`) — Probability $\ge 70\%$, urgent criminal conspiracy alert.
    - Moderate-Alert GNN Hypothesis Edge: Amber Dashed (`#f59e0b`, line-style: `dashed`, line-dash-pattern: `[6, 3]`, width: `2.5px`) — Probability $< 70\%$, investigative lead.

---

## 4. Primary User Flows & Interaction Specifications

### Flow A: 3-Stage Direct S3 Document Upload
The backend offloads binary traffic by issuing 300-second presigned S3 PUT URLs. The frontend MUST orchestrate the state machine without confusing the user:

```
[User drops file]
       │
       ▼
1. Initiate Session ──> POST /api/documents/upload/initiate
       │                Body: { title, description, file_name, case_id, document_type }
       │                Response: { document_id, upload_url, object_key }
       ▼
2. Direct S3 Upload ──> PUT to upload_url (Raw Binary)
       │                Tracks: onUploadProgress (0% -> 100%)
       │
       ├─── [On Success] ───> 3. Confirm ──> POST /api/documents/upload/confirm
       │                                     Body: { document_id, success: true }
       │
       └─── [On Failure / Abort] ──────────> POST /api/documents/upload/confirm
                                             Body: { document_id, success: false }
```

#### UX Best Practices for Upload:
1. **Immediate Optimistic Ingestion**: Display the document card immediately in the list in `pending` state with an inline circular or linear progress bar.
2. **Drag & Drop Target**: An expansive dashed dropzone on the case page that activates on hover.
3. **Modality Auto-Detection**:
   - `.png`, `.jpg`, `.jpeg`, `.webp`, `.pdf` (scanned) → auto-select `image`
   - `.txt`, `.csv`, `.docx`, `.json` → auto-select `text`
   - `.mp3`, `.wav`, `.m4a`, `.ogg`, `.aac` → auto-select `voice`
4. **Retry & Cancel**: If network breaks mid-PUT, provide a 1-click retry button before the 300-second expiration.

---

### Flow B: Case Management
1. **Create Case**: Quick modal or top-bar button (`+ New Case`). Requires non-empty name. POST to `/api/cases`.
2. **Rename Case**: Inline double-click or edit icon on case title trigger PUT `/api/cases/:id`.
3. **Delete Case**: Destructive dialog warning that cascade deletion will remove all associated documents. DELETE `/api/cases/:id`.

---

### Flow C: Deep Document Inspector (Split View)
When clicking a document, open a focused Split Workspace:
- **Left Pane (Media Viewer - 55% width)**:
  - For `image`: Pan, zoom (wheel/slider), rotate, fit-to-width, invert colors (useful for faded photocopied FIRs).
  - For `voice`: HTML5 custom audio player with waveform visualizer, playback speed (`0.75x`, `1x`, `1.25x`, `1.5x`, `2x`), jump ±5s, and loop.
  - For `text`: Formatted markdown / plaintext viewer with search in document.
  - Top Action Bar: Secure Download button (fetches `GET /api/documents/:id/download`, opens temporary signed link), Delete Document button.
- **Right Pane (Intelligence & Extracted Information - 45% width)**:
  - If `status === "processing"`: Shimmer skeleton card with message *"AI is parsing this document..."* and automatic poll every 3 seconds.
  - If `status === "finish"` and `extracted_information` is present:
    - Formatted OCR key-value cards (e.g., Police Station, FIR No., Complainant, Accused, Acts/Sections, Incident Date/Time).
    - Raw JSON toggle tab with syntax highlighting and 1-click "Copy JSON".
  - If `extracted_information === null`: Empty state card indicating pending processing or manual review trigger.

---

### Flow D: Investigation Knowledge Graph & Conspiracy Hypothesis Analysis

Investigators navigate between evidentiary document feeds and the relational Knowledge Graph (`/cases/:caseId/graph` or embedded tab).

```
+---------------------------------------------------------------------------------------------------+
| Case: FIR 101/2026 - Financial Fraud Syndicate       [Recompute Syndicate GNN ⚡] [Export Canvas]  |
+---------------------------------------------------------------------------------------------------+
|  [Cytoscape.js Interactive Canvas - cose / cola layout]            |  [Hypothesis Inspector]      |
|                                                                    |                              |
|       (Rajesh Sharma) ───────solid──────> [HDFC-501004128912]      |  Target: Rajesh Sharma       |
|              :                                                     |  Accomplice: Vikram Malhotra |
|              : dashed (Red - 82.4%)                                |  Edge: co_conspirator       |
|              v                                                     |  Confidence: 82.4%           |
|       (Vikram Malhotra) <────solid─────── [Phone: 9871987654]      |  Status: Hypothesis Flagged  |
|                                                                    |                              |
|                                                                    |  Legal Recommendation:       |
|                                                                    |  "Immediate interrogation    |
|                                                                    |   under BNS Section 61.      |
|                                                                    |   Subject routes funds via   |
|                                                                    |   common accounts."          |
|                                                                    |                              |
|                                                                    |  [Mark Verified Lead]        |
+---------------------------------------------------------------------------------------------------+
```

1. **Canvas Initialization**:
   - Fetch `GET /api/cases/:caseId/graph` (or `GET /api/gnn/graph` for global syndicate view).
   - Feed `response.elements` directly into Cytoscape.js (`cytoscape({ container, elements: response.elements, style, layout: { name: 'cose' } })`).
2. **Node Click Interaction**:
   - Highlight 1-hop and 2-hop connected neighbors, dimming unselected nodes to opacity `0.15`.
   - Open Right-Side Inspector showing entity details, badge (`Suspect`, `Candidate Associate`, `Financial Institution`), and extracted document attributes.
3. **Edge Click Interaction**:
   - **Evidentiary Edges (Solid `#64748b`)**: Display source document title and link to open the document viewer.
   - **Hypothesis Edges (Dashed `#ef4444` / `#f59e0b`)**: Display the GNN Prediction Inspector showing:
     - Predicted Relationship (`co_conspirator`, `shell_handler`)
     - Model Probability Badge (`82.4% Confidence`)
     - Statutory Recommendation Box (e.g. BNS Section 61 / IPC 120B)
     - Action button to add finding to official case diary.
4. **Trigger GNN Re-computation**:
   - Provide a persistent button `[Recompute Syndicate GNN ⚡]` on the graph toolbar.
   - Triggers `POST /api/gnn/trigger`.
   - Displays toast: *"GNN collective inference running across all cases..."*, then automatically refetches the updated graph.

---

## 5. API Integration Layer (TypeScript Specifications)

### TypeScript Data Models

```typescript
export type DocumentStatus = 'pending' | 'processing' | 'success' | 'failed' | 'finish';
export type DocumentType = 'image' | 'text' | 'voice';

export interface Document {
  id: string; // UUID v4
  title: string;
  description: string;
  status: DocumentStatus;
  document_type: DocumentType;
  object_key: string;
  extracted_information: Record<string, any> | null;
  case_id: string;
  created_at: string; // ISO 8601
  updated_at: string; // ISO 8601
}

export interface Case {
  id: string; // UUID v4
  name: string;
  created_at: string;
  updated_at: string;
}

export interface InitiateUploadPayload {
  title: string;
  description: string;
  file_name: string;
  case_id: string;
  document_type: DocumentType;
}

export interface InitiateUploadResponse {
  document_id: string;
  upload_url: string;
  object_key: string;
}

export type EntityNodeType = 'person' | 'phone' | 'financial_account' | 'object' | 'phantom_entity';

export interface NodeData {
  id: string;
  label: string;
  type: EntityNodeType;
  badge?: string;
  risk_score?: number;
  attributes?: Record<string, any>;
}

export interface CytoscapeNode {
  data: NodeData;
}

export interface EdgeData {
  id: string;
  source: string;
  target: string;
  label: string;
  category: 'evidentiary' | 'hypothesis';
  is_hypothesis: boolean;
  style: 'solid' | 'dashed';
  color: string;
  probability?: number;
  recommendation?: string;
  source_document?: string;
}

export interface CytoscapeEdge {
  data: EdgeData;
}

export interface GraphMeta {
  total_nodes: number;
  total_edges: number;
  evidentiary_edges: number;
  predicted_edges: number;
  last_analyzed: string;
}

export interface CytoscapeGraphResponse {
  status: 'success';
  case_id: string | null;
  meta: GraphMeta;
  elements: {
    nodes: CytoscapeNode[];
    edges: CytoscapeEdge[];
  };
}

export const cytoscapeDarkStylesheet = [
  {
    selector: 'node',
    style: {
      'label': 'data(label)',
      'color': '#f8fafc',
      'font-size': '11px',
      'text-valign': 'bottom',
      'text-margin-y': 4,
      'background-color': '#1e293b',
      'border-width': 2,
      'border-color': '#64748b',
    }
  },
  {
    selector: 'node[type = "person"]',
    style: { 'border-color': '#38bdf8', 'background-color': '#0f172a' }
  },
  {
    selector: 'node[type = "phone"]',
    style: { 'border-color': '#34d399', 'background-color': '#064e3b' }
  },
  {
    selector: 'node[type = "financial_account"]',
    style: { 'border-color': '#fbbf24', 'background-color': '#78350f' }
  },
  {
    selector: 'edge',
    style: {
      'curve-style': 'bezier',
      'target-arrow-shape': 'triangle',
      'label': 'data(label)',
      'font-size': '9px',
      'color': '#94a3b8',
      'text-rotation': 'autorotate',
      'line-color': 'data(color)',
      'target-arrow-color': 'data(color)',
      'width': 2,
    }
  },
  {
    selector: 'edge[is_hypothesis = true]',
    style: {
      'line-style': 'dashed',
      'line-dash-pattern': [6, 3],
      'width': 3,
    }
  }
];
```

### Complete S3 Direct Upload Hook Example

```typescript
import { useState } from 'react';
import axios from 'axios';

interface UseDirectUploadOptions {
  apiBaseUrl?: string;
  onSuccess?: (doc: Document) => void;
  onError?: (err: Error) => void;
}

export function useDirectUpload({ apiBaseUrl = 'http://localhost:8000', onSuccess, onError }: UseDirectUploadOptions = {}) {
  const [progress, setProgress] = useState<number>(0);
  const [stage, setStage] = useState<'idle' | 'initiating' | 'uploading' | 'confirming' | 'completed' | 'error'>('idle');

  const upload = async (file: File, metadata: Omit<InitiateUploadPayload, 'file_name'>) => {
    try {
      setStage('initiating');
      setProgress(5);

      // Step 1: Initiate upload session with server
      const { data: session } = await axios.post<InitiateUploadResponse>(
        `${apiBaseUrl}/api/documents/upload/initiate`,
        {
          ...metadata,
          file_name: file.name,
        }
      );

      setStage('uploading');

      // Step 2: Directly upload raw file binary to S3 presigned PUT URL
      await axios.put(session.upload_url, file, {
        headers: {
          'Content-Type': file.type || 'application/octet-stream',
        },
        onUploadProgress: (progressEvent) => {
          if (progressEvent.total) {
            const percent = Math.round((progressEvent.loaded * 90) / progressEvent.total) + 5;
            setProgress(percent);
          }
        },
      });

      setStage('confirming');

      // Step 3: Notify server that S3 upload was successful
      const { data: confirmedDoc } = await axios.post<Document>(
        `${apiBaseUrl}/api/documents/upload/confirm`,
        {
          document_id: session.document_id,
          success: true,
        }
      );

      setStage('completed');
      setProgress(100);
      onSuccess?.(confirmedDoc);
      return confirmedDoc;
    } catch (err: any) {
      setStage('error');
      onError?.(err);
      throw err;
    }
  };

  return { upload, progress, stage, isUploading: stage === 'initiating' || stage === 'uploading' || stage === 'confirming' };
}
```

---

## 6. Edge Cases, Failure Handling & Accessibility

1. **S3 Presigned URL Expiry (300s / 5 mins)**:
   - For slow network connections, if the upload takes > 4.5 minutes, alert the user or offer instant chunking/retry.
2. **Confirm Upload Failure Handling**:
   - If the client drops connection immediately after the S3 PUT before calling `/api/documents/upload/confirm`, the document remains in `pending` status.
   - The UI should display an alert badge on `pending` documents older than 5 minutes: *"Upload confirmation pending — [Verify S3 Status]"*.
3. **Empty States**:
   - Empty Case list: Illustration with "Create your first investigation case".
   - Empty Document list in a case: Dropzone invitation with keyboard shortcut hint (`⌘+U`).
4. **Keyboard Accessibility**:
   - `Esc`: Close modals and drawers.
   - `J` / `K` or `Up` / `Down`: Navigate document table rows.
   - `Space`: Quick Preview modal of selected document.
   - `Cmd/Ctrl + F`: Focus in-document OCR search.

---

## 7. Delivery Checklist for Frontend Team

- [ ] Configure Tailwind CSS semantic token mappings (matching Dark/Light standards).
- [ ] Implement `apiClient` using Axios/Fetch with standard error handler matching `{ error: string }`.
- [ ] Implement `useDirectUpload` state machine with progress reporting.
- [ ] Build `CaseSelector` and `CaseManagementModal` (CRUD).
- [ ] Build `DocumentFilterBar` (filter by `case_id`, `document_type`, `status`, text search).
- [ ] Build `SplitDocumentViewer` with Image Zoom/Pan, Audio Player, and OCR Information viewer.
- [ ] Implement short-polling or WebSocket observer for documents transitioning from `processing` to `finish`.
- [ ] Build `CytoscapeCanvas` component using `cytoscapeDarkStylesheet` with responsive auto-fit and zoom controls.
- [ ] Build `HypothesisInspector` drawer displaying link probability, relationship tags, and statutory legal recommendations.
- [ ] Implement `[Recompute Syndicate GNN ⚡]` trigger button calling `POST /api/gnn/trigger`.
- [ ] Build `/syndicate-graph` global view for cross-case organized crime syndicate visualization.
