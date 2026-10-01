//! Records with named representations (server v0.12+).
//!
//! A record is ONE primary dense vector -- the one that is indexed and
//! searched -- plus named extra representations (per-token or per-patch
//! multivectors for late interaction) stored beside it and deleted with it
//! through the ordinary vector routes (there is no record delete route).
//!
//! Opt-in on the server: without `DAKERA_RECORDS` both routes answer
//! `501 FEATURE_DISABLED` ([`ClientError::FeatureDisabled`](crate::ClientError)).
//! `DakeraClient::capabilities()` reports `records.enabled` and the limits.
//! v0.11 servers do not have the routes (`404`).
//!
//! ```rust,no_run
//! use dakera_client::{BlockDType, DakeraClient, RecordInput, RepresentationInput};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = DakeraClient::new("http://localhost:3000")?;
//! let record = RecordInput::new("r1", vec![0.1, 0.2, 0.3, 0.4])
//!     .with_representation(
//!         RepresentationInput::token_multivector("tokens", vec![vec![0.1, 0.2], vec![0.3, 0.4]])
//!             .with_store_as(BlockDType::F16),
//!     );
//! client.upsert_records("docs", vec![record]).await?;
//! let view = client.get_record("docs", "r1", false).await?;
//! println!("{} representations", view.representations.len());
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::types::{BlockDType, RepresentationKind};
use crate::DakeraClient;

/// One extra representation on the write path: `vectors.len()` rows of equal length.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepresentationInput {
    /// Slot name, unique within the record and never `"dense"` (reserved for
    /// the primary).  Slots the server derives itself (the document FDE of a
    /// late-interaction slot) must not be sent.
    pub name: String,
    /// `dense`, `token_multivector` or `patch_multivector`.
    pub kind: RepresentationKind,
    /// Registry wire name of the model that produced the vectors; empty means
    /// the namespace's default model.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// The vectors, row-major.  All rows must have the same, non-zero length.
    pub vectors: Vec<Vec<f32>>,
    /// How the server packs the block on disk: `f32` (lossless, default),
    /// `f16` (half the bytes) or `i8` (one scale per block).
    #[serde(default)]
    pub store_as: BlockDType,
}

impl RepresentationInput {
    /// A representation of `kind`.
    pub fn new(name: impl Into<String>, kind: RepresentationKind, vectors: Vec<Vec<f32>>) -> Self {
        Self {
            name: name.into(),
            kind,
            model: String::new(),
            vectors,
            store_as: BlockDType::F32,
        }
    }

    /// A `token_multivector` representation (late interaction over tokens).
    pub fn token_multivector(name: impl Into<String>, vectors: Vec<Vec<f32>>) -> Self {
        Self::new(name, RepresentationKind::TokenMultivector, vectors)
    }

    /// A `patch_multivector` representation (visual late interaction).
    pub fn patch_multivector(name: impl Into<String>, vectors: Vec<Vec<f32>>) -> Self {
        Self::new(name, RepresentationKind::PatchMultivector, vectors)
    }

    /// A `dense` extra representation.
    pub fn dense(name: impl Into<String>, vectors: Vec<Vec<f32>>) -> Self {
        Self::new(name, RepresentationKind::Dense, vectors)
    }

    /// Set the on-disk dtype.
    pub fn with_store_as(mut self, store_as: BlockDType) -> Self {
        self.store_as = store_as;
        self
    }

    /// Set the producing model's wire name.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

/// One record on the write path: a primary vector plus its extras.  A record
/// with no representations is exactly a plain vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordInput {
    /// Record id.
    pub id: String,
    /// The primary dense vector (indexed and searched).
    pub values: Vec<f32>,
    /// Extra named representations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub representations: Vec<RepresentationInput>,
    /// Free-form metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Seconds after which the record expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

impl RecordInput {
    /// A record with a primary vector and no extras.
    pub fn new(id: impl Into<String>, values: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            values,
            representations: Vec::new(),
            metadata: None,
            ttl_seconds: None,
        }
    }

    /// Add a named representation.
    pub fn with_representation(mut self, representation: RepresentationInput) -> Self {
        self.representations.push(representation);
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set the TTL in seconds.
    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = Some(ttl_seconds);
        self
    }
}

/// Body of `POST /v1/namespaces/{ns}/records`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordUpsertRequest {
    /// The records (1 to 10 000 per request, one namespace dimension).
    pub records: Vec<RecordInput>,
}

/// Response of a record upsert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordUpsertResponse {
    /// Number of records written (a record is one vector with extras).
    pub upserted_count: usize,
}

/// What a record carries in one slot; the payload only with `include_vectors`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepresentationInfo {
    /// Slot name.
    pub name: String,
    /// Slot kind.
    pub kind: RepresentationKind,
    /// Producing model, when recorded.
    #[serde(default)]
    pub model: String,
    /// Width of one row.
    #[serde(default)]
    pub dim: u32,
    /// Number of rows.
    #[serde(default)]
    pub count: u32,
    /// Stored dtype.
    #[serde(default)]
    pub dtype: BlockDType,
    /// Packed size on disk, in bytes.
    #[serde(default)]
    pub bytes: usize,
    /// The decoded rows; present only with `include_vectors = true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vectors: Option<Vec<Vec<f32>>>,
}

/// Response of `GET /v1/namespaces/{ns}/records/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordView {
    /// Record id.
    pub id: String,
    /// The primary vector; present only with `include_vectors = true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<f32>>,
    /// Dimension of the primary (always reported).
    #[serde(default)]
    pub dimension: usize,
    /// Extra representations (empty for a plain dense record).
    #[serde(default)]
    pub representations: Vec<RepresentationInfo>,
    /// Slots the server could not read (a kind or dtype written by a newer
    /// Dakera); skipped and counted, never dropped from storage.
    #[serde(default)]
    pub unsupported_representations: usize,
    /// Metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// TTL in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    /// Absolute expiry (Unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

impl DakeraClient {
    /// Write records (`POST /v1/namespaces/{namespace}/records`).
    ///
    /// A record over `DAKERA_RECORD_MAX_BYTES` / `DAKERA_RECORD_MAX_VECTORS`
    /// is [`ClientError::PayloadTooLarge`](crate::ClientError); a server
    /// without `DAKERA_RECORDS` answers
    /// [`ClientError::FeatureDisabled`](crate::ClientError).
    pub async fn upsert_records(
        &self,
        namespace: &str,
        records: Vec<RecordInput>,
    ) -> Result<RecordUpsertResponse> {
        let url = format!("{}/v1/namespaces/{}/records", self.base_url, namespace);
        let request = RecordUpsertRequest { records };
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Read one record (`GET /v1/namespaces/{namespace}/records/{id}`).
    ///
    /// Returns a manifest of the representations; with `include_vectors` the
    /// primary values and every representation's decoded rows too (a token
    /// block is 20 to 100 times the dense vector, so it is off by default).
    pub async fn get_record(
        &self,
        namespace: &str,
        id: &str,
        include_vectors: bool,
    ) -> Result<RecordView> {
        let url = format!(
            "{}/v1/namespaces/{}/records/{}?include_vectors={}",
            self.base_url, namespace, id, include_vectors
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }
}
