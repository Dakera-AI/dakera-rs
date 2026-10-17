//! Document import (server v0.12.3+, Dakera-AI/dakera#923).
//!
//! A PDF, DOCX, HTML, Markdown or text document becomes a set of *chunk
//! memories* of an agent: ordinary memories, each sized for the embedding
//! model, carrying its heading breadcrumb and its provenance under the
//! `_dakera_document` metadata key ([`DocumentChunkMeta`]). Recall, search and
//! filters treat them as memories; a result can cite the document, page and
//! section ([`RecalledMemory::document_chunk`]).
//!
//! Shapes follow the server's `crates/common/src/types/document.rs` and
//! `documents/routes.rs`. A server with `DAKERA_DOCUMENTS` off answers `501`
//! ([`ClientError::FeatureDisabled`]); a server older than v0.12.3 answers
//! `404`, which these methods report as such. Check
//! [`ServerCapabilities::supports_documents`](crate::ServerCapabilities::supports_documents)
//! first to avoid the round trip.
//!
//! ```rust,no_run
//! use dakera_client::{DakeraClient, ImportDocumentRequest};
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = DakeraClient::new("http://localhost:3000")?;
//! let accepted = client
//!     .import_document(
//!         ImportDocumentRequest::new("docs-bot", "# Manual\n\nInstall it.")
//!             .with_media_type("text/markdown"),
//!     )
//!     .await?;
//! let job = client
//!     .wait_for_document_job("docs-bot", &accepted.job_id, Duration::from_secs(300))
//!     .await?;
//! assert!(job.is_completed(), "{:?}", job.error);
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{ClientError, Result, ServerErrorCode};
use crate::memory::{MemoryType, RecalledMemory};
use crate::types::JobInfo;
use crate::DakeraClient;

/// User-metadata key of a chunk memory (see [`DocumentChunkMeta`]).
pub const DOCUMENT_META_KEY: &str = "_dakera_document";

/// Prefix of the tag every chunk of a document carries: `document:<id>`.
pub const DOCUMENT_TAG_PREFIX: &str = "document:";

/// Job type of document imports in `/ops/jobs`.
pub const DOCUMENT_IMPORT_JOB_TYPE: &str = "document_import";

/// Media types [`DakeraClient::import_document`] accepts as text.
pub const DOCUMENT_TEXT_MEDIA_TYPES: &[&str] = &["text/plain", "text/markdown", "text/html"];

/// How the extracted text is split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkStrategy {
    /// Sections at headings, then paragraphs, sentences, words (the default).
    Structure,
    /// Plain token windows with overlap; headings are text.
    Fixed,
    /// A strategy this SDK version does not know (a newer server sent it) —
    /// never a deserialisation error. Do not send it.
    #[serde(other)]
    Unknown,
}

/// The `chunking` field of an import. The server refuses unknown fields in
/// it; unset fields are omitted (server defaults:
/// `DAKERA_DOCUMENT_CHUNK_TOKENS`, `DAKERA_DOCUMENT_CHUNK_OVERLAP_TOKENS`,
/// `structure`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkingOptions {
    /// Token budget of one chunk's text (clamped to the embedding model's cap).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    /// Tokens repeated from the end of the previous chunk (at most a quarter
    /// of the budget).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlap_tokens: Option<usize>,
    /// `structure` (default) or `fixed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<ChunkStrategy>,
}

/// The fields both imports share: what every chunk memory carries (as
/// `StoreMemoryRequest`), the chunking and `decompose`. Unset fields are
/// omitted, so the server defaults apply.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentImportOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<MemoryType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Language of the document for the write-time derivations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunking: Option<ChunkingOptions>,
    /// Derive sentence sub-memories from every chunk (server default
    /// `false`: a chunk is already a retrieval unit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decompose: Option<bool>,
}

/// Body of `POST /v1/memory/documents`: a document given as text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDocumentRequest {
    pub agent_id: String,
    /// The document itself (at most `DAKERA_DOCUMENT_MAX_BYTES` bytes).
    pub content: String,
    /// `text/plain` (server default), `text/markdown` or `text/html`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// Wins over the document's own title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Default: derived from the agent and the content hash, so importing the
    /// same bytes again replaces the document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Memory fields, chunking and `decompose`.
    #[serde(flatten)]
    pub options: DocumentImportOptions,
}

impl ImportDocumentRequest {
    /// Import `content` (plain text unless [`Self::with_media_type`]) for `agent_id`.
    pub fn new(agent_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            content: content.into(),
            media_type: None,
            title: None,
            document_id: None,
            options: DocumentImportOptions::default(),
        }
    }

    /// `text/plain`, `text/markdown` or `text/html`.
    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    /// The document's title (wins over the document's own).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The document id (a re-import with the same id replaces the document).
    pub fn with_document_id(mut self, document_id: impl Into<String>) -> Self {
        self.document_id = Some(document_id.into());
        self
    }

    /// Memory fields, chunking and `decompose`.
    pub fn with_options(mut self, options: DocumentImportOptions) -> Self {
        self.options = options;
        self
    }
}

/// Body of `POST /v1/namespaces/{ns}/attachments/{ref}/document`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportAttachmentDocumentRequest {
    pub agent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Default: derived from the agent and the attachment's content hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// Memory fields, chunking and `decompose`.
    #[serde(flatten)]
    pub options: DocumentImportOptions,
}

impl ImportAttachmentDocumentRequest {
    /// Import the attachment for `agent_id`.
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            title: None,
            document_id: None,
            options: DocumentImportOptions::default(),
        }
    }

    /// The document's title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The document id.
    pub fn with_document_id(mut self, document_id: impl Into<String>) -> Self {
        self.document_id = Some(document_id.into());
        self
    }

    /// Memory fields, chunking and `decompose`.
    pub fn with_options(mut self, options: DocumentImportOptions) -> Self {
        self.options = options;
        self
    }
}

/// `202` answer of both imports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportDocumentAccepted {
    pub job_id: String,
    pub document_id: String,
    pub agent_id: String,
    /// Route to poll for the job.
    pub status_url: String,
}

/// What `_dakera_document` holds on every chunk memory. The document-level
/// fields are repeated on every chunk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentChunkMeta {
    /// The document id.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub media_type: String,
    /// Position of this chunk, from 0.
    pub chunk_index: usize,
    /// Chunks of the document.
    pub chunk_count: usize,
    /// First and last page the chunk's text comes from (PDF; form-feed pages
    /// of plain text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_start: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_end: Option<u32>,
    /// Headings above the chunk, outermost first (the title included).
    #[serde(default)]
    pub heading_path: Vec<String>,
    /// Character range of the chunk's text in the extracted text.
    pub char_start: usize,
    pub char_end: usize,
    /// SHA-256 (hex) of the source document's bytes.
    pub sha256: String,
    /// Bytes of the source document.
    #[serde(default)]
    pub bytes: usize,
    /// Pages of the source document, when it has pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<u32>,
    /// First and last import (Unix seconds).
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    /// Sentence sub-memories are derived from this chunk.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub decompose: bool,
    /// Hash of everything the chunk memory was written from.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fingerprint: String,
}

impl DocumentChunkMeta {
    /// The `_dakera_document` of a memory's metadata, when it is a chunk.
    pub fn from_metadata(metadata: Option<&serde_json::Value>) -> Option<Self> {
        let value = metadata?.get(DOCUMENT_META_KEY)?;
        serde_json::from_value(value.clone()).ok()
    }
}

impl RecalledMemory {
    /// Where this memory comes from when it is a document chunk (document,
    /// pages, heading path); `None` otherwise.
    pub fn document_chunk(&self) -> Option<DocumentChunkMeta> {
        DocumentChunkMeta::from_metadata(self.metadata.as_ref())
    }
}

/// One document of [`DakeraClient::list_documents`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub document_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub media_type: String,
    pub chunk_count: usize,
    pub bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<u32>,
    pub created_at: u64,
    pub updated_at: u64,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_ref: Option<String>,
}

/// `GET /v1/memory/documents` answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentListResponse {
    pub documents: Vec<DocumentSummary>,
    /// Pass as `cursor` for the next page; `None` on the last one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// One chunk of [`DakeraClient::get_document`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentChunkInfo {
    /// The chunk's memory id.
    pub id: String,
    pub chunk_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_start: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_end: Option<u32>,
    #[serde(default)]
    pub heading_path: Vec<String>,
    pub char_start: usize,
    pub char_end: usize,
}

/// `GET /v1/memory/documents/{id}` answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentDetail {
    pub document: DocumentSummary,
    pub chunks: Vec<DocumentChunkInfo>,
}

/// `DELETE /v1/memory/documents/{id}` answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteDocumentResponse {
    pub document_id: String,
    /// Chunk memories removed.
    pub deleted_chunks: usize,
}

/// Make a `404` from a server without the document routes say why.
fn route_error(err: ClientError) -> ClientError {
    match err {
        ClientError::Server {
            status: 404,
            message,
            code: Some(ServerErrorCode::RouteNotFound),
        } => ClientError::Server {
            status: 404,
            message: format!(
                "{message} (document import needs Dakera server >= 0.12.3; \
                 see ServerCapabilities::supports_documents)"
            ),
            code: Some(ServerErrorCode::RouteNotFound),
        },
        other => other,
    }
}

impl DakeraClient {
    /// Import a document given as text (`text/plain`, `text/markdown` or
    /// `text/html`) — `POST /v1/memory/documents`, write scope on the agent.
    /// Returns the `202` job: follow it with [`Self::document_job`] or
    /// [`Self::wait_for_document_job`].
    pub async fn import_document(
        &self,
        request: ImportDocumentRequest,
    ) -> Result<ImportDocumentAccepted> {
        let url = format!("{}/v1/memory/documents", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await.map_err(route_error)
    }

    /// Import an uploaded attachment (PDF, DOCX, HTML, Markdown or text, by
    /// its stored media type) — `POST
    /// /v1/namespaces/{namespace}/attachments/{attachment_ref}/document`, read
    /// on `namespace` and write on the agent. Upload it first with
    /// [`Self::upload_attachment`]; needs `DAKERA_ATTACHMENTS` too.
    pub async fn import_attachment_document(
        &self,
        namespace: &str,
        attachment_ref: &str,
        request: ImportAttachmentDocumentRequest,
    ) -> Result<ImportDocumentAccepted> {
        let url = format!(
            "{}/v1/namespaces/{}/attachments/{}/document",
            self.base_url,
            crate::client::enc(namespace),
            crate::client::enc(attachment_ref)
        );
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await.map_err(route_error)
    }

    /// A document import job — `GET /v1/memory/documents/jobs/{job_id}?agent_id=`.
    /// `status` is `Pending`, `Running` (`progress`, `message`
    /// `chunks_stored n/total`), `Completed` or `Failed` (`error` holds the
    /// status and code: `413`, `422 UNPROCESSABLE_CONTENT`,
    /// `504 EXTRACTION_TIMEOUT`, ...).
    pub async fn document_job(&self, agent_id: &str, job_id: &str) -> Result<JobInfo> {
        let url = format!(
            "{}/v1/memory/documents/jobs/{}?agent_id={}",
            self.base_url,
            crate::client::enc(job_id),
            crate::client::enc_query(agent_id)
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await.map_err(route_error)
    }

    /// Wait for a document import job to end, polling once a second.
    ///
    /// Returns the final [`JobInfo`] when it is `Completed`, `Failed` or
    /// `Cancelled` (a failed job is `Ok`: check [`JobInfo::is_failed`] and
    /// `error`), [`ClientError::Timeout`] after `timeout`. A `503` is waited
    /// out for its `Retry-After`; any other error is returned.
    pub async fn wait_for_document_job(
        &self,
        agent_id: &str,
        job_id: &str,
        timeout: Duration,
    ) -> Result<JobInfo> {
        self.wait_for_document_job_every(agent_id, job_id, timeout, Duration::from_secs(1))
            .await
    }

    /// [`Self::wait_for_document_job`] with the delay between polls.
    pub async fn wait_for_document_job_every(
        &self,
        agent_id: &str,
        job_id: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<JobInfo> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let pause = match self.document_job(agent_id, job_id).await {
                Ok(job) if job.is_finished() => return Ok(job),
                Ok(_) => poll_interval,
                Err(e @ ClientError::ServiceUnavailable { .. }) => {
                    e.retry_after().unwrap_or(poll_interval)
                }
                Err(e) => return Err(e),
            };
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(ClientError::Timeout);
            }
            tokio::time::sleep(pause.min(remaining)).await;
        }
    }

    /// An agent's documents, sorted by id — `GET
    /// /v1/memory/documents?agent_id=&cursor=&limit=`. `limit` 1–500 (server
    /// default 50); pass the previous page's `next_cursor` as `cursor`.
    pub async fn list_documents(
        &self,
        agent_id: &str,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<DocumentListResponse> {
        let mut url = format!(
            "{}/v1/memory/documents?agent_id={}",
            self.base_url,
            crate::client::enc_query(agent_id)
        );
        if let Some(cursor) = cursor {
            url.push_str(&format!("&cursor={}", crate::client::enc_query(cursor)));
        }
        if let Some(limit) = limit {
            url.push_str(&format!("&limit={limit}"));
        }
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await.map_err(route_error)
    }

    /// One document and its chunks — `GET /v1/memory/documents/{document_id}?agent_id=`.
    /// An unknown document is a `404` (`ClientError::Server`).
    pub async fn get_document(&self, agent_id: &str, document_id: &str) -> Result<DocumentDetail> {
        let url = format!(
            "{}/v1/memory/documents/{}?agent_id={}",
            self.base_url,
            crate::client::enc(document_id),
            crate::client::enc_query(agent_id)
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await.map_err(route_error)
    }

    /// Delete every chunk of a document — `DELETE
    /// /v1/memory/documents/{document_id}?agent_id=`, write scope. Its
    /// attachment goes when no memory references it any more.
    pub async fn delete_document(
        &self,
        agent_id: &str,
        document_id: &str,
    ) -> Result<DeleteDocumentResponse> {
        let url = format!(
            "{}/v1/memory/documents/{}?agent_id={}",
            self.base_url,
            crate::client::enc(document_id),
            crate::client::enc_query(agent_id)
        );
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await.map_err(route_error)
    }
}
