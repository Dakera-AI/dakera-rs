//! R9 / DAK-10004 — forward-compat: lenient enums + GET /v1/capabilities.
//!
//! Contract under test (server `routes/capabilities.rs`): every field additive;
//! unknown fields and unknown strings inside lists MUST be ignored; the SDK must
//! never fail deserialisation on a model / index kind / search mode / metric /
//! representation kind / dtype string it does not know.

use dakera_client::{
    parse_accepted_values, BlockDType, CapabilityKind, ClientError, ConfigureNamespaceRequest,
    ConfigureNamespaceResponse, CreateNamespaceRequest, DakeraClient, DistanceMetric,
    EmbeddingModel, IndexKind, QueryTextRequest, RepresentationKind, SearchMode,
    ServerCapabilities, TextDocument, TextUpsertResponse, UpsertTextRequest,
};

// A capabilities document as the v0.12 server emits it, PLUS a model string and
// an index kind this SDK does not know, PLUS an extra top-level field, an extra
// model field and an extra records field — all of which must be tolerated.
const FIXTURE: &str = r#"{
  "capabilities_version": 1,
  "server_version": "0.12.0",
  "api_versions": ["v1"],
  "default_model": "bge-large",
  "models": [
    {"name": "bge-large", "aliases": ["bge-large-en", "bge-large-en-v1.5"], "dimension": 1024,
     "max_seq_length": 512, "effective_max_seq_length": 512, "active": true, "modality": "text"},
    {"name": "modernbert-embed-base", "aliases": ["modernbert", "modern-bert"], "dimension": 768,
     "max_seq_length": 8192, "effective_max_seq_length": 2048, "active": false,
     "mrl_dimensions": [256, 768], "modality": "text"},
    {"name": "bge-m3", "aliases": ["bge-m3-dense", "baai/bge-m3"], "dimension": 1024,
     "max_seq_length": 8192, "effective_max_seq_length": 2048, "active": false, "modality": "text"},
    {"name": "colmodernvbert-v9", "aliases": [], "dimension": 128, "max_seq_length": 4096,
     "effective_max_seq_length": 4096, "active": false, "modality": "image", "quantised": true}
  ],
  "index_kinds": ["hnsw", "pq", "ivf", "ivfpq", "spfresh", "fulltext", "muvera_fde"],
  "vector_index_kinds": ["hnsw", "ivf", "ivfpq", "spfresh", "muvera_fde"],
  "live_vector_index_kinds": ["hnsw", "ivf", "spfresh"],
  "distance_metrics": ["cosine", "euclidean", "dot_product", "hamming"],
  "search_mode": "rabitq",
  "search_modes_accepted": "hybrid, binary, float, scalar (alias sq), rabitq, warp9",
  "fulltext_language": "de",
  "on_disk_format_version": 1,
  "records": {
    "enabled": true,
    "representation_kinds": ["dense", "token_multivector", "patch_multivector", "holo"],
    "dtypes": ["f32", "f16", "i8", "e4m3"],
    "max_representations": 8, "max_vectors": 4096, "max_bytes": 8388608,
    "compression": "zstd"
  },
  "query_languages": ["en", "de", "fr", "es", "it", "pt", "nl"],
  "reembed_pending": true,
  "future_top_level_field": {"anything": [1, 2, 3]}
}"#;

const UPSERT_TEXT_UNKNOWN_MODEL: &str = r#"{"upserted_count":1,"tokens_processed":4,"model":"colmodernvbert-v9","embedding_time_ms":3,"new_field_from_future":true}"#;

// ============================================================================
// 1. Lenient enums
// ============================================================================

#[test]
fn unknown_enum_strings_deserialise_as_unknown_and_round_trip() {
    let model: EmbeddingModel = serde_json::from_str(r#""colmodernvbert-v9""#).unwrap();
    assert_eq!(model, EmbeddingModel::Unknown("colmodernvbert-v9".into()));
    assert!(!model.is_known());
    assert_eq!(model.as_str(), "colmodernvbert-v9");
    assert_eq!(
        serde_json::to_string(&model).unwrap(),
        r#""colmodernvbert-v9""#
    );
    assert_eq!(model.to_string(), "colmodernvbert-v9");

    // Known values are unchanged, including the v0.12 additions.
    let m3: EmbeddingModel = serde_json::from_str(r#""bge-m3""#).unwrap();
    assert_eq!(m3, EmbeddingModel::BgeM3);
    assert!(m3.is_known());
    assert_eq!(
        EmbeddingModel::from("modernbert-embed-base"),
        EmbeddingModel::ModernBertEmbedBase
    );
    assert_eq!(EmbeddingModel::default(), EmbeddingModel::BgeLarge);
    assert!(EmbeddingModel::known().contains(&EmbeddingModel::BgeM3));
    assert!(IndexKind::known().contains(&IndexKind::IvfPq));
    assert!(SearchMode::known().contains(&SearchMode::RaBitQ));
    assert_eq!(
        RepresentationKind::known(),
        vec![
            RepresentationKind::Dense,
            RepresentationKind::TokenMultivector,
            RepresentationKind::PatchMultivector,
        ]
    );
    assert_eq!(
        BlockDType::known(),
        vec![BlockDType::F32, BlockDType::F16, BlockDType::I8]
    );

    let kind: IndexKind = serde_json::from_str(r#""muvera_fde""#).unwrap();
    assert_eq!(kind, IndexKind::Unknown("muvera_fde".into()));
    let mode: SearchMode = serde_json::from_str(r#""warp9""#).unwrap();
    assert!(!mode.is_known());
    let repr: RepresentationKind = serde_json::from_str(r#""holo""#).unwrap();
    assert_eq!(repr.as_str(), "holo");
    let dtype: BlockDType = serde_json::from_str(r#""e4m3""#).unwrap();
    assert_eq!(String::from(dtype), "e4m3");

    // Every lenient enum CARRIES the wire string it did not recognise.
    let metric: DistanceMetric = serde_json::from_str(r#""hamming""#).unwrap();
    assert_eq!(metric, DistanceMetric::Unknown("hamming".into()));
    assert_eq!(metric.as_str(), "hamming", "the wire string must survive");
    assert!(!metric.is_known());
    assert_eq!(
        DistanceMetric::from("dot_product"),
        DistanceMetric::DotProduct
    );
    let routing: dakera_client::RoutingMode = serde_json::from_str(r#""graph""#).unwrap();
    assert!(!routing.is_known());
}

#[test]
fn responses_carrying_unknown_enum_strings_deserialise() {
    let up: TextUpsertResponse = serde_json::from_str(UPSERT_TEXT_UNKNOWN_MODEL).unwrap();
    assert_eq!(up.model.as_str(), "colmodernvbert-v9");
    assert!(!up.model.is_known());
    let cfg: ConfigureNamespaceResponse = serde_json::from_str(
        r#"{"namespace":"ns","dimension":8,"distance":"hamming","created":true}"#,
    )
    .unwrap();
    assert_eq!(cfg.distance, DistanceMetric::Unknown("hamming".into()));
}

/// R9 regression: a metric the SERVER advertises but this SDK does not name must
/// pass pre-flight.
///
/// `DistanceMetric` used to stay `Copy` and drop the unrecognised wire string.
/// `Capabilities::supported_values` renders the server's list through
/// `as_str()`, so the fixture's `"hamming"` became the literal `"unknown"` — and
/// the pre-flight then did the exact opposite of its job in BOTH directions: it
/// accepted `"unknown"`, which no server advertises, and refused `"hamming"`,
/// which this one does. A client could not reach a new server metric at all,
/// which is worse than having no pre-flight, because the call never leaves.
#[test]
fn a_server_metric_this_sdk_does_not_name_is_still_supported() {
    let caps: ServerCapabilities = serde_json::from_str(FIXTURE).unwrap();

    // The fixture advertises cosine, euclidean, dot_product and hamming.
    assert!(caps.supports(CapabilityKind::DistanceMetric, "hamming"));
    assert!(caps.supports(CapabilityKind::DistanceMetric, "cosine"));

    // "unknown" is not a metric; it was only ever an artefact of the old enum.
    assert!(!caps.supports(CapabilityKind::DistanceMetric, "unknown"));
    assert!(!caps.supports(CapabilityKind::DistanceMetric, "manhattan"));

    // And the advertised list reads back as the server's own strings.
    let values = caps.supported_values(CapabilityKind::DistanceMetric);
    assert!(values.contains(&"hamming".to_string()), "{values:?}");
    assert!(!values.contains(&"unknown".to_string()), "{values:?}");
}

#[tokio::test]
async fn upsert_text_with_unknown_model_in_response_succeeds() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/ns/upsert-text")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(UPSERT_TEXT_UNKNOWN_MODEL)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .upsert_text(
            "ns",
            UpsertTextRequest::new(vec![TextDocument::new("d", "t")])
                .with_model(EmbeddingModel::from("bge-m3")),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.model,
        EmbeddingModel::Unknown("colmodernvbert-v9".into())
    );
    mock.assert_async().await;
}

// ============================================================================
// 2. ServerCapabilities parsing
// ============================================================================

#[test]
fn fixture_with_unknown_strings_and_unknown_fields_parses() {
    let caps: ServerCapabilities = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(caps.capabilities_version, 1);
    assert_eq!(caps.server_version, "0.12.0");
    assert_eq!(caps.api_versions, vec!["v1"]);
    assert_eq!(caps.default_model, EmbeddingModel::BgeLarge);
    assert_eq!(
        caps.model_names(),
        vec![
            "bge-large",
            "modernbert-embed-base",
            "bge-m3",
            "colmodernvbert-v9"
        ]
    );
    let unknown = caps
        .model("colmodernvbert-v9")
        .expect("unknown model row kept");
    assert!(!unknown.name.is_known());
    assert_eq!(unknown.modality, "image");
    assert_eq!(unknown.extra["quantised"], serde_json::json!(true));
    assert_eq!(
        caps.model("modernbert").unwrap().mrl_dimensions,
        Some(vec![256, 768])
    );
    assert_eq!(caps.model("bge-large").unwrap().mrl_dimensions, None);
    assert_eq!(caps.active_model().unwrap().name, EmbeddingModel::BgeLarge);
    assert!(caps.index_kinds.contains(&IndexKind::IvfPq));
    assert!(caps
        .index_kinds
        .contains(&IndexKind::Unknown("muvera_fde".into())));
    assert_eq!(
        caps.live_vector_index_kinds,
        vec![IndexKind::Hnsw, IndexKind::Ivf, IndexKind::SpFresh]
    );
    assert!(caps
        .distance_metrics
        .contains(&DistanceMetric::Unknown("hamming".into())));
    assert_eq!(caps.search_mode, SearchMode::RaBitQ);
    assert_eq!(
        caps.search_modes_accepted,
        vec![
            SearchMode::Hybrid,
            SearchMode::Binary,
            SearchMode::Float,
            SearchMode::Scalar,
            SearchMode::Unknown("sq".into()),
            SearchMode::RaBitQ,
            SearchMode::Unknown("warp9".into()),
        ]
    );
    assert_eq!(caps.fulltext_language, "de");
    assert_eq!(caps.on_disk_format_version, 1);
    assert!(caps.supports_records());
    assert!(caps
        .records
        .representation_kinds
        .contains(&RepresentationKind::Unknown("holo".into())));
    assert!(caps
        .records
        .dtypes
        .contains(&BlockDType::Unknown("e4m3".into())));
    assert_eq!(caps.records.max_bytes, 8 * 1024 * 1024);
    assert_eq!(caps.records.extra["compression"], serde_json::json!("zstd"));
    assert_eq!(caps.query_languages.last().map(String::as_str), Some("nl"));
    assert!(caps.reembed_pending);
    assert_eq!(
        caps.extra["future_top_level_field"],
        serde_json::json!({"anything": [1, 2, 3]})
    );
}

#[test]
fn accepted_values_parser_and_support_helpers() {
    assert_eq!(
        parse_accepted_values("hybrid, binary, float, scalar (alias sq), rabitq"),
        vec!["hybrid", "binary", "float", "scalar", "sq", "rabitq"]
    );
    let caps: ServerCapabilities = serde_json::from_str(FIXTURE).unwrap();
    assert!(caps.supports(CapabilityKind::Model, "bge-m3"));
    assert!(caps.supports(CapabilityKind::Model, "baai/bge-m3")); // alias
    assert!(!caps.supports(CapabilityKind::Model, "minilm"));
    assert!(caps.supports(CapabilityKind::IndexKind, "ivfpq"));
    assert!(!caps.supports(CapabilityKind::IndexKind, "flat"));
    assert!(caps.supports(CapabilityKind::DistanceMetric, "cosine"));
    assert!(caps.supports(CapabilityKind::SearchMode, "sq"));
    assert!(!caps.supports(CapabilityKind::SearchMode, "exact"));
    assert!(caps.supports(CapabilityKind::QueryLanguage, "de"));
    assert!(!caps.supports(CapabilityKind::QueryLanguage, "ja"));
    assert!(caps.require(CapabilityKind::Model, "bge-m3").is_ok());

    match caps.require(CapabilityKind::Model, "minilm").unwrap_err() {
        ClientError::UnsupportedCapability {
            kind,
            requested,
            supported,
            server_version,
        } => {
            assert_eq!(kind, CapabilityKind::Model);
            assert_eq!(requested, "minilm");
            assert_eq!(
                supported,
                vec![
                    "bge-large",
                    "modernbert-embed-base",
                    "bge-m3",
                    "colmodernvbert-v9"
                ]
            );
            assert_eq!(server_version, "0.12.0");
        }
        other => panic!("expected UnsupportedCapability, got {other:?}"),
    }
    let message = caps
        .require(CapabilityKind::Model, "minilm")
        .unwrap_err()
        .to_string();
    assert!(message.contains("minilm"), "{message}");
    assert!(message.contains("bge-m3"), "{message}");
    assert!(message.contains("v0.12.0"), "{message}");
    assert!(!caps
        .require(CapabilityKind::Model, "minilm")
        .unwrap_err()
        .is_retryable());
}

// ============================================================================
// 3. client.capabilities() + pre-flight
// ============================================================================

#[tokio::test]
async fn capabilities_are_fetched_once_and_cached_then_refreshed() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/capabilities")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(FIXTURE)
        .expect(2)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();

    let first = client.capabilities().await.unwrap();
    let second = client.capabilities().await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert!(first.reembed_pending);
    assert!(first.supports_records());

    // A clone shares the cache; refresh replaces it for both.
    let cloned = client.clone();
    let third = cloned.refresh_capabilities().await.unwrap();
    assert!(!std::sync::Arc::ptr_eq(&first, &third));
    assert!(std::sync::Arc::ptr_eq(
        &third,
        &client.capabilities().await.unwrap()
    ));
    mock.assert_async().await;
}

#[tokio::test]
async fn pre_012_server_returns_not_found() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/v1/capabilities")
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"not found"}"#)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let err = client.capabilities().await.unwrap_err();
    assert!(err.is_not_found(), "{err:?}");
}

#[tokio::test]
async fn preflight_rejects_unsupported_values_before_sending() {
    let mut server = mockito::Server::new_async().await;
    let caps_mock = server
        .mock("GET", "/v1/capabilities")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(FIXTURE)
        .expect(1)
        .create_async()
        .await;
    let never = server
        .mock("POST", mockito::Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let never_put = server
        .mock("PUT", mockito::Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client.capabilities().await.unwrap(); // populate the cache; no builder flag needed

    let err = client
        .upsert_text(
            "ns",
            UpsertTextRequest::new(vec![TextDocument::new("d", "t")])
                .with_model(EmbeddingModel::Minilm),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ClientError::UnsupportedCapability {
            kind: CapabilityKind::Model,
            ..
        }
    ));
    let err = client
        .query_text(
            "ns",
            QueryTextRequest::new("q", 3).with_model(EmbeddingModel::Minilm),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::UnsupportedCapability { .. }));
    let create = CreateNamespaceRequest {
        dimensions: Some(8),
        index_type: Some("flat".to_string()),
        metadata: None,
    };
    let err = client.create_namespace("ns", create).await.unwrap_err();
    assert!(matches!(
        err,
        ClientError::UnsupportedCapability {
            kind: CapabilityKind::IndexKind,
            ..
        }
    ));
    let err = client
        .configure_namespace(
            "ns",
            ConfigureNamespaceRequest {
                dimension: 8,
                // A metric the fixture does NOT advertise. Before DistanceMetric
                // carried its string this was unexpressible: every unknown metric
                // rendered as the literal "unknown", which the fixture's own
                // "hamming" row also rendered as, so this check passed vacuously
                // in one direction and failed in the other.
                distance: Some(DistanceMetric::Unknown("manhattan".into())),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ClientError::UnsupportedCapability {
            kind: CapabilityKind::DistanceMetric,
            ..
        }
    ));

    caps_mock.assert_async().await;
    never.assert_async().await;
    never_put.assert_async().await;
}

#[tokio::test]
async fn preflight_lets_supported_model_and_alias_through() {
    let mut server = mockito::Server::new_async().await;
    let _caps = server
        .mock("GET", "/v1/capabilities")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(FIXTURE)
        .create_async()
        .await;
    let upserts = server
        .mock("POST", "/v1/namespaces/ns/upsert-text")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(UPSERT_TEXT_UNKNOWN_MODEL)
        .expect(2)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client.capabilities().await.unwrap();
    for model in ["bge-m3", "baai/bge-m3"] {
        client
            .upsert_text(
                "ns",
                UpsertTextRequest::new(vec![TextDocument::new("d", "t")])
                    .with_model(EmbeddingModel::from(model)),
            )
            .await
            .unwrap();
    }
    upserts.assert_async().await;
}

#[tokio::test]
async fn require_supported_covers_search_mode_and_query_language() {
    let mut server = mockito::Server::new_async().await;
    let _caps = server
        .mock("GET", "/v1/capabilities")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(FIXTURE)
        .expect(1)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .require_supported(CapabilityKind::SearchMode, "rabitq")
        .await
        .unwrap();
    // "sq" is an alias expanded from the prose `search_modes_accepted` field.
    client
        .require_supported(CapabilityKind::SearchMode, "sq")
        .await
        .unwrap();
    let err = client
        .require_supported(CapabilityKind::SearchMode, "exact")
        .await
        .unwrap_err();
    match err {
        ClientError::UnsupportedCapability { supported, .. } => assert_eq!(
            supported,
            vec!["hybrid", "binary", "float", "scalar", "sq", "rabitq", "warp9"]
        ),
        other => panic!("unexpected {other:?}"),
    }
    client
        .require_supported(CapabilityKind::QueryLanguage, "fr")
        .await
        .unwrap();
    assert!(client
        .require_supported(CapabilityKind::QueryLanguage, "ja")
        .await
        .is_err());
}

#[tokio::test]
async fn preflight_is_off_by_default_without_a_cache() {
    let mut server = mockito::Server::new_async().await;
    let never = server
        .mock("GET", "/v1/capabilities")
        .expect(0)
        .create_async()
        .await;
    let upsert = server
        .mock("POST", "/v1/namespaces/ns/upsert-text")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(UPSERT_TEXT_UNKNOWN_MODEL)
        .expect(1)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .upsert_text(
            "ns",
            UpsertTextRequest::new(vec![TextDocument::new("d", "t")])
                .with_model(EmbeddingModel::Minilm),
        )
        .await
        .unwrap();
    never.assert_async().await;
    upsert.assert_async().await;
}

#[tokio::test]
async fn builder_preflight_fetches_lazily() {
    let mut server = mockito::Server::new_async().await;
    let caps = server
        .mock("GET", "/v1/capabilities")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(FIXTURE)
        .expect(1)
        .create_async()
        .await;
    let never = server
        .mock("POST", mockito::Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let client = DakeraClient::builder(server.url())
        .preflight(true)
        .build()
        .unwrap();
    let err = client
        .upsert_text(
            "ns",
            UpsertTextRequest::new(vec![TextDocument::new("d", "t")])
                .with_model(EmbeddingModel::Minilm),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::UnsupportedCapability { .. }));
    caps.assert_async().await;
    never.assert_async().await;
}

#[tokio::test]
async fn builder_preflight_degrades_silently_on_pre_012_server() {
    let mut server = mockito::Server::new_async().await;
    let caps = server
        .mock("GET", "/v1/capabilities")
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"not found"}"#)
        .expect(1) // asked once, then never again for this client
        .create_async()
        .await;
    let upserts = server
        .mock("POST", "/v1/namespaces/ns/upsert-text")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(UPSERT_TEXT_UNKNOWN_MODEL)
        .expect(2)
        .create_async()
        .await;
    let client = DakeraClient::builder(server.url())
        .preflight(true)
        .build()
        .unwrap();
    for _ in 0..2 {
        client
            .upsert_text(
                "ns",
                UpsertTextRequest::new(vec![TextDocument::new("d", "t")])
                    .with_model(EmbeddingModel::Minilm),
            )
            .await
            .unwrap();
    }
    caps.assert_async().await;
    upserts.assert_async().await;
}
