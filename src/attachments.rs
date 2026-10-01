//! Attachments, speech-to-text and image indexing (server v0.12+).
//!
//! Files an agent's memories can point at.  They are stored content-addressed
//! per namespace (`attachment_ref` = `sha256:<hex of the bytes>`), counted by
//! namespace quotas and removed with the last memory that references them.
//!
//! Everything here is opt-in on the server: a server that did not set
//! `DAKERA_ATTACHMENTS` answers every route with `501 FEATURE_DISABLED`
//! ([`ClientError::FeatureDisabled`], whose message names the variable), and
//! the image routes need `DAKERA_VISION` as well.  Check
//! [`ServerCapabilities::attachments`](crate::ServerCapabilities) first to
//! avoid the round trip.  v0.11 servers do not have these routes (`404`).
//!
//! ```rust,no_run
//! use dakera_client::{DakeraClient, TranscribeRequest};
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = DakeraClient::new("http://localhost:3000")?;
//! let wav = std::fs::read("note.wav")?;
//! let up = client.upload_attachment("uploads", wav, "audio/wav").await?;
//! let job = client
//!     .transcribe_attachment("uploads", &up.attachment_ref, TranscribeRequest::new("my-agent"))
//!     .await?;
//! let done = client
//!     .wait_for_attachment_job(&job.status_url, Duration::from_secs(300))
//!     .await?;
//! println!("{} -> memory {}", done.status, job.memory_id);
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use reqwest::header::{CONTENT_TYPE, ETAG};
use serde::{Deserialize, Serialize};

use crate::error::{ClientError, Result};
use crate::memory::MemoryType;
use crate::types::JobInfo;
use crate::DakeraClient;

/// Response of `POST /v1/namespaces/{ns}/attachments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentUploadResponse {
    /// `sha256:<hex>` -- what a memory's `attachment_ref` carries.
    pub attachment_ref: String,
    /// Media type the attachment is stored with.
    pub content_type: String,
    /// Size of the stored bytes.
    pub size_bytes: u64,
    /// `false` when the namespace already held these very bytes (the upload
    /// was a no-op and the existing media type was kept).
    #[serde(default)]
    pub created: bool,
}

/// One entry of `GET /v1/namespaces/{ns}/attachments` (no bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentEntry {
    /// `sha256:<hex>`.
    pub attachment_ref: String,
    /// Media type.
    pub content_type: String,
    /// Size in bytes.
    pub size_bytes: u64,
}

/// Response of `GET /v1/namespaces/{ns}/attachments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentListResponse {
    /// The namespace's attachments.
    #[serde(default)]
    pub attachments: Vec<AttachmentEntry>,
}

/// The bytes of a downloaded attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentDownload {
    /// Media type the attachment was uploaded with.
    pub content_type: String,
    /// The `ETag` (the content hash), quotes removed, when present.
    pub etag: Option<String>,
    /// The content.
    pub data: Vec<u8>,
}

/// `202` body of the transcribe and image-index routes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentJobAccepted {
    /// Job id (`job_<run>_<n>`).
    pub job_id: String,
    /// The attachment being processed.
    pub attachment_ref: String,
    /// The agent whose memory the job stores.
    pub agent_id: String,
    /// Id of the memory the job stores.  Jobs live in server memory: after a
    /// restart the job is unknown (`404 JOB_NOT_FOUND`) and this id is how to
    /// find what a completed job stored.
    #[serde(default)]
    pub memory_id: String,
    /// Wire name of the model the job runs (`whisper-tiny.en`, `colmodernvbert`).
    #[serde(default)]
    pub model: String,
    /// Route to poll (`GET`, relative to the server's base URL).
    pub status_url: String,
}

/// `202` body of `POST .../attachments/{ref}/transcribe`.
pub type TranscribeAccepted = AttachmentJobAccepted;
/// `202` body of `POST .../attachments/{ref}/index`.
pub type IndexImageAccepted = AttachmentJobAccepted;

/// Body of `POST /v1/namespaces/{ns}/attachments/{ref}/transcribe`: the memory
/// the transcript becomes, minus its content (that is the transcript).
///
/// The key needs read on the attachment's namespace and write on the agent's.
/// The attachment is copied into the agent's namespace.  WAV audio only (PCM
/// or float, any rate and channel count); the model is English-only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscribeRequest {
    /// Agent that owns the stored memory (required).
    pub agent_id: String,
    /// Memory type (server default: episodic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<MemoryType>,
    /// Session to attach the memory to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Importance (server default 0.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Free-form metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Seconds after which the memory expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    /// Unix timestamp at which the memory expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Custom memory id; omitted derives a stable one, so the same request
    /// sent again stores the same memory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Language of the transcript for the write-time derivations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

impl TranscribeRequest {
    /// A request storing the transcript as a memory of `agent_id`.
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            ..Default::default()
        }
    }

    /// Set the memory type.
    pub fn with_type(mut self, memory_type: MemoryType) -> Self {
        self.memory_type = Some(memory_type);
        self
    }

    /// Set the importance (clamped to 0.0..=1.0).
    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = Some(importance.clamp(0.0, 1.0));
        self
    }

    /// Set the tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set the session.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set a custom memory id.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the language of the transcript.
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = Some(lang.into());
        self
    }
}

/// Body of `POST /v1/namespaces/{ns}/attachments/{ref}/index`: same fields as
/// [`TranscribeRequest`], plus the text that stands in for the page.
///
/// Needs `DAKERA_VISION` and `DAKERA_ATTACHMENTS`.  PNG only, at most 64
/// megapixels.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexImageRequest {
    /// Agent that owns the stored memory (required).
    pub agent_id: String,
    /// Caption / title stored as the memory's text (full-text index and
    /// display only -- the vectors come from the pixels).  Server default:
    /// `[image sha256:...]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Memory type (server default: episodic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<MemoryType>,
    /// Session to attach the memory to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Importance (server default 0.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Free-form metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Seconds after which the memory expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    /// Unix timestamp at which the memory expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Custom memory id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Language of `content` for the write-time derivations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

impl IndexImageRequest {
    /// A request storing the indexed image as a memory of `agent_id`.
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            ..Default::default()
        }
    }

    /// Set the caption stored as the memory's text.
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set the tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set a custom memory id.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the language of `content`.
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = Some(lang.into());
        self
    }
}

impl JobInfo {
    /// The job reached a terminal state (`Completed`, `Failed` or `Cancelled`).
    pub fn is_finished(&self) -> bool {
        matches!(self.status.as_str(), "Completed" | "Failed" | "Cancelled")
    }

    /// The job completed successfully.
    pub fn is_completed(&self) -> bool {
        self.status == "Completed"
    }

    /// The job failed; see [`JobInfo::error`] and `message`.
    pub fn is_failed(&self) -> bool {
        self.status == "Failed"
    }
}

impl DakeraClient {
    /// Upload an attachment (`POST /v1/namespaces/{namespace}/attachments`).
    ///
    /// Sent as a raw body with `content_type` (for example `audio/wav`,
    /// `image/png`).  `201` for new bytes, `200` when the namespace already
    /// held them (`created == false`).  A body over
    /// `DAKERA_ATTACHMENT_MAX_BYTES` (25 MiB by default) is
    /// [`ClientError::PayloadTooLarge`]; a hard namespace quota is
    /// [`ClientError::QuotaExceeded`].
    pub async fn upload_attachment(
        &self,
        namespace: &str,
        data: impl Into<Vec<u8>>,
        content_type: &str,
    ) -> Result<AttachmentUploadResponse> {
        let url = format!("{}/v1/namespaces/{}/attachments", self.base_url, namespace);
        let body: Vec<u8> = data.into();
        let response = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await?;
        self.handle_response(response).await
    }

    /// List a namespace's attachments (`GET /v1/namespaces/{namespace}/attachments`).
    pub async fn list_attachments(&self, namespace: &str) -> Result<AttachmentListResponse> {
        let url = format!("{}/v1/namespaces/{}/attachments", self.base_url, namespace);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Download an attachment's bytes
    /// (`GET /v1/namespaces/{namespace}/attachments/{attachment_ref}`).
    pub async fn download_attachment(
        &self,
        namespace: &str,
        attachment_ref: &str,
    ) -> Result<AttachmentDownload> {
        let url = format!(
            "{}/v1/namespaces/{}/attachments/{}",
            self.base_url, namespace, attachment_ref
        );
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(Self::error_from_response(response).await);
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim_matches('"').to_string());
        let data = response.bytes().await?.to_vec();
        Ok(AttachmentDownload {
            content_type,
            etag,
            data,
        })
    }

    /// Delete an attachment
    /// (`DELETE /v1/namespaces/{namespace}/attachments/{attachment_ref}`).
    ///
    /// A `409` while a memory still references it (forget the memory
    /// instead) comes back as `ClientError::Server { status: 409, .. }`.
    pub async fn delete_attachment(&self, namespace: &str, attachment_ref: &str) -> Result<()> {
        let url = format!(
            "{}/v1/namespaces/{}/attachments/{}",
            self.base_url, namespace, attachment_ref
        );
        let response = self.client.delete(&url).send().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(Self::error_from_response(response).await)
        }
    }

    /// Start a speech-to-text job
    /// (`POST /v1/namespaces/{namespace}/attachments/{attachment_ref}/transcribe`).
    ///
    /// Returns `202` with the job to poll.  Anything but WAV audio is a `400`
    /// before a job exists.
    pub async fn transcribe_attachment(
        &self,
        namespace: &str,
        attachment_ref: &str,
        request: TranscribeRequest,
    ) -> Result<TranscribeAccepted> {
        let url = format!(
            "{}/v1/namespaces/{}/attachments/{}/transcribe",
            self.base_url, namespace, attachment_ref
        );
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Status of a transcription job
    /// (`GET .../attachments/{attachment_ref}/transcribe/{job_id}`).
    ///
    /// A job the server does not know (it restarted: jobs live in memory) is
    /// a 404 with code `JOB_NOT_FOUND`; the memory a completed job stored is
    /// kept, look it up by the `memory_id` the `202` returned.
    pub async fn transcription_status(
        &self,
        namespace: &str,
        attachment_ref: &str,
        job_id: &str,
    ) -> Result<JobInfo> {
        let url = format!(
            "{}/v1/namespaces/{}/attachments/{}/transcribe/{}",
            self.base_url, namespace, attachment_ref, job_id
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Start an image-indexing job
    /// (`POST /v1/namespaces/{namespace}/attachments/{attachment_ref}/index`).
    ///
    /// Needs `DAKERA_VISION` and `DAKERA_ATTACHMENTS` on the server.
    pub async fn index_image_attachment(
        &self,
        namespace: &str,
        attachment_ref: &str,
        request: IndexImageRequest,
    ) -> Result<IndexImageAccepted> {
        let url = format!(
            "{}/v1/namespaces/{}/attachments/{}/index",
            self.base_url, namespace, attachment_ref
        );
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Status of an image-indexing job
    /// (`GET .../attachments/{attachment_ref}/index/{job_id}`).
    pub async fn index_image_status(
        &self,
        namespace: &str,
        attachment_ref: &str,
        job_id: &str,
    ) -> Result<JobInfo> {
        let url = format!(
            "{}/v1/namespaces/{}/attachments/{}/index/{}",
            self.base_url, namespace, attachment_ref, job_id
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Poll the `status_url` a transcribe / index `202` returned.
    pub async fn attachment_job_status(&self, status_url: &str) -> Result<JobInfo> {
        let url = format!("{}{}", self.base_url, status_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Wait for a transcribe / index job to finish.
    ///
    /// Polls `status_url` once a second until the job is `Completed`,
    /// `Failed` or `Cancelled` and returns its final [`JobInfo`] (a failed
    /// job is `Ok` with `error` set: check [`JobInfo::is_failed`]).  Returns
    /// [`ClientError::Timeout`] after `timeout`.  A transient `503` is waited
    /// out for its `Retry-After`; any other error is returned.
    pub async fn wait_for_attachment_job(
        &self,
        status_url: &str,
        timeout: Duration,
    ) -> Result<JobInfo> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let pause = match self.attachment_job_status(status_url).await {
                Ok(job) if job.is_finished() => return Ok(job),
                Ok(_) => Duration::from_secs(1),
                Err(e @ ClientError::ServiceUnavailable { .. }) => {
                    e.retry_after().unwrap_or(Duration::from_secs(1))
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
}
