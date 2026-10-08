//! `GET /v1/capabilities` — what the connected server can do (server v0.12+).
//!
//! R9 / DAK-10004.  Contract (server `routes/capabilities.rs`): every field is
//! additive; unknown fields and unknown strings inside lists MUST be ignored;
//! `capabilities_version` bumps only on a breaking reshape of the document.
//! Every field here is `#[serde(default)]`, unknown fields are collected into
//! `extra`, and every list element is a lenient enum, so a newer server never
//! breaks deserialisation.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{ClientError, Result};
use crate::types::{
    BlockDType, DistanceMetric, EmbeddingModel, IndexKind, RepresentationKind, SearchMode,
};

/// One embedding model the server can load (`capabilities.models[]`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModelCapability {
    /// Wire name — the string accepted/returned in every `model` field.
    #[serde(default)]
    pub name: EmbeddingModel,
    /// Other spellings accepted on input.
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub dimension: usize,
    /// The model's own context window (tokens).
    #[serde(default)]
    pub max_seq_length: usize,
    /// What THIS server embeds before truncating.
    #[serde(default)]
    pub effective_max_seq_length: usize,
    /// Whether this is the model the server embeds with (one per store).
    #[serde(default)]
    pub active: bool,
    /// Matryoshka truncation dimensions, when supported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mrl_dimensions: Option<Vec<usize>>,
    #[serde(default = "default_modality")]
    pub modality: String,
    /// Fields this SDK version does not model yet (kept, never rejected).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_modality() -> String {
    "text".to_string()
}

impl ModelCapability {
    /// Whether `name` is this model's wire name or one of its aliases.
    pub fn matches(&self, name: &str) -> bool {
        self.name.as_str() == name || self.aliases.iter().any(|a| a == name)
    }
}

/// The R2 record / representation surface (`capabilities.records`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RecordCapabilities {
    /// Whether `/v1/namespaces/{ns}/records` answers (else 501 FEATURE_DISABLED).
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub representation_kinds: Vec<RepresentationKind>,
    #[serde(default)]
    pub dtypes: Vec<BlockDType>,
    #[serde(default)]
    pub max_representations: usize,
    #[serde(default)]
    pub max_vectors: usize,
    #[serde(default)]
    pub max_bytes: usize,
    /// Fields this SDK version does not model yet.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// F3 / R4: the scoring axis and the late-interaction lane (`capabilities.scoring`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ScoringCapability {
    /// `DAKERA_SCORING_STRATEGY` as configured (`single-vector` unless set).
    #[serde(default)]
    pub strategy: String,
    /// Prose list of accepted strategies.
    #[serde(default)]
    pub strategies_accepted: String,
    #[serde(default)]
    pub late_interaction: LateInteractionCapability,
    /// Fields this SDK version does not model yet.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The late-interaction (MaxSim) lane (`capabilities.scoring.late_interaction`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LateInteractionCapability {
    /// The strategy selects late interaction.
    #[serde(default)]
    pub enabled: bool,
    /// The active model has a late-interaction recipe (else writes are refused).
    #[serde(default)]
    pub model_supported: bool,
    /// `text` or `visual`.
    #[serde(default)]
    pub lane: String,
    #[serde(default)]
    pub token_slot: String,
    #[serde(default)]
    pub fde_slot: String,
    #[serde(default)]
    pub candidates: usize,
    /// Fields this SDK version does not model yet (the MUVERA parameters, ...).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The speech-to-text surface (`capabilities.attachments.transcription`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionCapability {
    /// Wire name of the model a transcription runs (`DAKERA_WHISPER_MODEL`).
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub models: Vec<String>,
    /// Media types the transcriber decodes.
    #[serde(default)]
    pub media_types: Vec<String>,
    /// Languages (ISO 639-1) the configured model transcribes.
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub sample_rate_hz: u32,
    /// Fields this SDK version does not model yet.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The attachment lane (`capabilities.attachments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AttachmentCapability {
    /// Whether the attachment routes answer (`DAKERA_ATTACHMENTS`); else 501 `FEATURE_DISABLED`.
    #[serde(default)]
    pub enabled: bool,
    /// Bytes one upload may carry; over it the upload is a 413.
    #[serde(default)]
    pub max_bytes: usize,
    #[serde(default)]
    pub transcription: TranscriptionCapability,
    /// Fields this SDK version does not model yet.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The visual late-interaction lane (`capabilities.vision`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VisionCapability {
    /// Whether `POST .../attachments/{ref}/index` answers (`DAKERA_VISION`).
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub models: Vec<String>,
    /// Media types the image decoder accepts.
    #[serde(default)]
    pub media_types: Vec<String>,
    /// Width of one patch vector.
    #[serde(default)]
    pub dimension: usize,
    #[serde(default)]
    pub patch_slot: String,
    #[serde(default)]
    pub patch_fde_slot: String,
    /// Fields this SDK version does not model yet (tile geometry, ...).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Key and grant features (`capabilities.auth`, server v0.12.2+).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AuthCapability {
    /// Key grants may hold `p*` prefix patterns.
    #[serde(default)]
    pub prefix_patterns: bool,
    /// Sessions are authorized by their agent's namespace alone (no
    /// `_dakera_sessions` grant needed).
    #[serde(default)]
    pub sessions_by_agent: bool,
    /// `PATCH /admin/keys/{id}` and `PATCH /v1/namespaces/{ns}/keys/{id}` exist.
    #[serde(default)]
    pub key_update: bool,
    /// Longest rotation grace period (seconds).
    #[serde(default)]
    pub rotation_grace_max_secs: u64,
    /// Most entries in a key's `namespaces`.
    #[serde(default)]
    pub max_grants: usize,
    /// Longest grant entry (bytes).
    #[serde(default)]
    pub max_grant_len: usize,
    /// Fields this SDK version does not model yet.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Naming rules (`capabilities.naming`, server v0.12.2+).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NamingCapability {
    /// Regular expression an agent id must match.
    #[serde(default)]
    pub agent_id_pattern: String,
    /// Longest agent id (bytes; 241 since v0.12.2).
    #[serde(default)]
    pub agent_id_max_bytes: usize,
    /// Prefix of an agent's memory namespace (`_dakera_agent_`).
    #[serde(default)]
    pub agent_namespace_prefix: String,
    /// Longest agent namespace (bytes).
    #[serde(default)]
    pub agent_namespace_max_bytes: usize,
    /// Regular expression a client namespace must match.
    #[serde(default)]
    pub namespace_pattern: String,
    /// Longest client namespace (bytes).
    #[serde(default)]
    pub namespace_max_bytes: usize,
    /// Prefixes a client namespace may not start with.
    #[serde(default)]
    pub reserved_prefixes: Vec<String>,
    /// Server-internal namespaces (refused on client routes).
    #[serde(default)]
    pub internal_namespaces: Vec<String>,
    /// Prefixes of server-internal namespace families.
    #[serde(default)]
    pub internal_prefixes: Vec<String>,
    /// Fields this SDK version does not model yet.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Session lifecycle (`capabilities.sessions`, server v0.12.2+).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionsCapability {
    /// The live server-wide inactivity timeout (seconds; `0` = sessions
    /// without their own timeout are never ended for inactivity).
    #[serde(default)]
    pub idle_timeout_secs: u64,
    /// Longest per-session `idle_timeout_secs` accepted at start.
    #[serde(default)]
    pub max_idle_timeout_secs: u64,
    /// `POST /v1/sessions/{id}/touch` exists.
    #[serde(default)]
    pub touch: bool,
    /// Sessions carry `ended_reason`.
    #[serde(default)]
    pub ended_reason: bool,
    /// Fields this SDK version does not model yet.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Parse an accepted-values field the server emits as prose —
/// `"hybrid, binary, float, scalar (alias sq), rabitq"` — so `x (alias y)`
/// yields both `x` and `y`.
pub fn parse_accepted_values(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = value;
    while !rest.is_empty() {
        let end = rest.find([',', '(']).unwrap_or(rest.len());
        let head = rest[..end].trim();
        if !head.is_empty() {
            out.push(head.to_string());
        }
        rest = &rest[end..];
        if let Some(inner) = rest.strip_prefix('(') {
            let close = inner.find(')').unwrap_or(inner.len());
            let note = inner[..close].trim();
            let lowered = note.to_ascii_lowercase();
            if lowered.starts_with("alias") {
                let aliases = note[5..].strip_prefix("es").unwrap_or(&note[5..]);
                out.extend(
                    aliases
                        .split(|c: char| c.is_whitespace() || c == ',' || c == '/')
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                );
            }
            rest = if close < inner.len() {
                &inner[close + 1..]
            } else {
                ""
            };
        }
        if let Some(after_comma) = rest.strip_prefix(',') {
            rest = after_comma;
        }
    }
    out
}

/// `search_modes_accepted` is prose today; accept a JSON list too in case the
/// field is ever reshaped, and treat any other shape as empty (ignore, never fail).
fn deserialize_accepted_modes<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<SearchMode>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Prose(String),
        List(Vec<String>),
        /// Anything else the server sends for this field. The payload is never
        /// read — it exists so the untagged match SUCCEEDS on an unexpected
        /// shape instead of failing the whole capabilities document, which is
        /// the forward-compatibility contract this deserializer is here for.
        Other(#[allow(dead_code)] serde_json::Value),
    }
    let values = match Raw::deserialize(deserializer)? {
        Raw::Prose(s) => parse_accepted_values(&s),
        Raw::List(list) => list,
        Raw::Other(_) => Vec::new(),
    };
    Ok(values.into_iter().map(SearchMode::from).collect())
}

/// The capabilities document.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ServerCapabilities {
    /// Shape version of this document (bumps only on a breaking reshape).
    #[serde(default)]
    pub capabilities_version: u32,
    /// Server semver.
    #[serde(default)]
    pub server_version: String,
    /// REST API prefixes this server mounts.
    #[serde(default)]
    pub api_versions: Vec<String>,
    /// Model new namespaces/memories are embedded with.
    #[serde(default)]
    pub default_model: EmbeddingModel,
    #[serde(default)]
    pub models: Vec<ModelCapability>,
    /// Index kinds this build knows (stable storage keys).
    #[serde(default)]
    pub index_kinds: Vec<IndexKind>,
    /// Subset of `index_kinds` that can serve vector queries.
    #[serde(default)]
    pub vector_index_kinds: Vec<IndexKind>,
    /// Subset the engine actually builds today.
    #[serde(default)]
    pub live_vector_index_kinds: Vec<IndexKind>,
    /// Accepted `distance_metric` values.
    #[serde(default)]
    pub distance_metrics: Vec<DistanceMetric>,
    /// The mode this server process runs (`DAKERA_SEARCH_MODE`).
    #[serde(default)]
    pub search_mode: SearchMode,
    /// Every value the server accepts for `DAKERA_SEARCH_MODE` (aliases expanded).
    #[serde(default, deserialize_with = "deserialize_accepted_modes")]
    pub search_modes_accepted: Vec<SearchMode>,
    #[serde(default)]
    pub fulltext_language: String,
    #[serde(default)]
    pub on_disk_format_version: u16,
    /// R2: the record / representation surface.
    #[serde(default)]
    pub records: RecordCapabilities,
    /// Languages (ISO 639-1) the per-request `lang` field is honoured for.
    #[serde(default)]
    pub query_languages: Vec<String>,
    /// A model change was acknowledged but the store is not fully re-embedded yet.
    #[serde(default)]
    pub reembed_pending: bool,
    /// F3 / R4: the scoring strategy and the late-interaction lane (v0.12).
    #[serde(default)]
    pub scoring: ScoringCapability,
    /// F5: attachments and speech-to-text (v0.12).
    #[serde(default)]
    pub attachments: AttachmentCapability,
    /// F4: the visual late-interaction lane (v0.12).
    #[serde(default)]
    pub vision: VisionCapability,
    /// Records this node skipped on read because a newer Dakera wrote them;
    /// non-zero means searches can return fewer results than the store holds.
    #[serde(default)]
    pub unreadable_records: u64,
    /// Live counters of the late-interaction path since start (name -> count).
    #[serde(default)]
    pub late_interaction_stats: HashMap<String, serde_json::Value>,
    /// Key and grant features (`capabilities_version` 2, server v0.12.2+;
    /// all-default from older servers).
    #[serde(default)]
    pub auth: AuthCapability,
    /// Naming rules (server v0.12.2+; all-default from older servers).
    #[serde(default)]
    pub naming: NamingCapability,
    /// Session lifecycle (server v0.12.2+; all-default from older servers).
    #[serde(default)]
    pub sessions: SessionsCapability,
    /// Top-level fields this SDK version does not model yet (kept, never rejected).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Registries a pre-flight check can validate against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityKind {
    /// `capabilities.models[].name` (aliases count).
    Model,
    /// `capabilities.index_kinds`.
    IndexKind,
    /// `capabilities.distance_metrics`.
    DistanceMetric,
    /// `capabilities.search_modes_accepted`.
    SearchMode,
    /// `capabilities.query_languages`.
    QueryLanguage,
}

impl CapabilityKind {
    /// The snake_case name used in error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            CapabilityKind::Model => "model",
            CapabilityKind::IndexKind => "index_kind",
            CapabilityKind::DistanceMetric => "distance_metric",
            CapabilityKind::SearchMode => "search_mode",
            CapabilityKind::QueryLanguage => "query_language",
        }
    }
}

impl std::fmt::Display for CapabilityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ServerCapabilities {
    /// Look a model up by wire name or alias.
    pub fn model(&self, name_or_alias: &str) -> Option<&ModelCapability> {
        self.models.iter().find(|m| m.matches(name_or_alias))
    }

    /// The model the server embeds with (a request naming another is rejected).
    pub fn active_model(&self) -> Option<&ModelCapability> {
        self.models.iter().find(|m| m.active)
    }

    /// Wire names of every model the server can load.
    pub fn model_names(&self) -> Vec<String> {
        self.models
            .iter()
            .map(|m| m.name.as_str().to_string())
            .collect()
    }

    /// Whether the server supports the v0.12.2 key edits (`PATCH` key routes).
    pub fn supports_key_update(&self) -> bool {
        self.auth.key_update
    }

    /// Whether the server supports `POST /v1/sessions/{id}/touch`.
    pub fn supports_session_touch(&self) -> bool {
        self.sessions.touch
    }

    /// Whether the record routes are switched on (`records.enabled`).
    pub fn supports_records(&self) -> bool {
        self.records.enabled
    }

    /// Wire strings the server advertises for `kind`.
    pub fn supported_values(&self, kind: CapabilityKind) -> Vec<String> {
        match kind {
            CapabilityKind::Model => self.model_names(),
            CapabilityKind::IndexKind => self
                .index_kinds
                .iter()
                .map(|k| k.as_str().to_string())
                .collect(),
            CapabilityKind::DistanceMetric => self
                .distance_metrics
                .iter()
                .map(|m| m.as_str().to_string())
                .collect(),
            CapabilityKind::SearchMode => self
                .search_modes_accepted
                .iter()
                .map(|m| m.as_str().to_string())
                .collect(),
            CapabilityKind::QueryLanguage => self.query_languages.clone(),
        }
    }

    /// Whether the server advertises `value` for `kind` (model aliases count).
    pub fn supports(&self, kind: CapabilityKind, value: &str) -> bool {
        match kind {
            CapabilityKind::Model => self.model(value).is_some(),
            _ => self.supported_values(kind).iter().any(|v| v == value),
        }
    }

    /// `Err(ClientError::UnsupportedCapability)` unless the server advertises
    /// `value` for `kind`.
    pub fn require(&self, kind: CapabilityKind, value: &str) -> Result<()> {
        if self.supports(kind, value) {
            return Ok(());
        }
        Err(ClientError::UnsupportedCapability {
            kind,
            requested: value.to_string(),
            supported: self.supported_values(kind),
            server_version: if self.server_version.is_empty() {
                "unknown".to_string()
            } else {
                self.server_version.clone()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_accepted_values_expand_aliases() {
        assert_eq!(
            parse_accepted_values("hybrid, binary, float, scalar (alias sq), rabitq"),
            vec!["hybrid", "binary", "float", "scalar", "sq", "rabitq"]
        );
        assert_eq!(parse_accepted_values("a (alias b, c)"), vec!["a", "b", "c"]);
        assert!(parse_accepted_values("").is_empty());
        assert_eq!(parse_accepted_values("x (unbalanced"), vec!["x"]);
    }

    #[test]
    fn accepted_modes_accept_prose_list_or_garbage() {
        let prose: ServerCapabilities =
            serde_json::from_str(r#"{"search_modes_accepted":"hybrid, scalar (alias sq)"}"#)
                .unwrap();
        assert_eq!(
            prose.search_modes_accepted,
            vec![
                SearchMode::Hybrid,
                SearchMode::Scalar,
                SearchMode::Unknown("sq".into())
            ]
        );
        let list: ServerCapabilities =
            serde_json::from_str(r#"{"search_modes_accepted":["hybrid","float"]}"#).unwrap();
        assert_eq!(
            list.search_modes_accepted,
            vec![SearchMode::Hybrid, SearchMode::Float]
        );
        let garbage: ServerCapabilities =
            serde_json::from_str(r#"{"search_modes_accepted":42}"#).unwrap();
        assert!(garbage.search_modes_accepted.is_empty());
        let empty: ServerCapabilities = serde_json::from_str("{}").unwrap();
        assert!(empty.models.is_empty());
        assert!(!empty.supports_records());
    }
}
