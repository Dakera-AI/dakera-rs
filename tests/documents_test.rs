//! Document import (Dakera-AI/dakera#923). Wire shapes from the server's
//! `crates/common/src/types/document.rs`, `documents/routes.rs` and the
//! `JobInfo` of `operations.rs`.

use std::time::Duration;

use dakera_client::memory::MemoryType;
use dakera_client::{
    ChunkStrategy, ChunkingOptions, ClientError, DakeraClient, DocumentImportOptions,
    ImportAttachmentDocumentRequest, ImportDocumentRequest, RecallRequest, RecallResponse,
    ServerCapabilities, ServerErrorCode,
};
use mockito::Matcher;
use serde_json::json;

const ACCEPTED: &str = r#"{"job_id":"job_1_7","document_id":"manual-v3","agent_id":"docs-bot",
    "status_url":"/v1/memory/documents/jobs/job_1_7?agent_id=docs-bot"}"#;

fn json_mock(
    server: &mut mockito::ServerGuard,
    method: &str,
    path: &str,
    status: usize,
    body: &str,
) -> mockito::Mock {
    server
        .mock(method, path)
        .with_status(status)
        .with_header("content-type", "application/json")
        .with_body(body)
}

#[tokio::test]
async fn import_text_minimal_body() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(&mut server, "POST", "/v1/memory/documents", 202, ACCEPTED)
        .match_body(Matcher::Json(
            json!({"agent_id": "docs-bot", "content": "# T\n\nHello"}),
        ))
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let acc = client
        .import_document(ImportDocumentRequest::new("docs-bot", "# T\n\nHello"))
        .await
        .unwrap();
    assert_eq!(acc.job_id, "job_1_7");
    assert_eq!(acc.document_id, "manual-v3");
    assert!(acc
        .status_url
        .starts_with("/v1/memory/documents/jobs/job_1_7"));
    m.assert_async().await;
}

#[tokio::test]
async fn import_text_all_fields() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(&mut server, "POST", "/v1/memory/documents", 202, ACCEPTED)
        .match_body(Matcher::Json(json!({
            "agent_id": "docs-bot", "content": "<h1>T</h1>", "media_type": "text/html",
            "title": "Manual", "document_id": "manual-v3", "memory_type": "semantic",
            "session_id": "s1", "importance": 0.75, "tags": ["manual"],
            "metadata": {"team": "ops"}, "ttl_seconds": 3600, "lang": "en",
            "chunking": {"max_tokens": 200, "overlap_tokens": 20, "strategy": "fixed"},
            "decompose": true
        })))
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .import_document(
            ImportDocumentRequest::new("docs-bot", "<h1>T</h1>")
                .with_media_type("text/html")
                .with_title("Manual")
                .with_document_id("manual-v3")
                .with_options(DocumentImportOptions {
                    memory_type: Some(MemoryType::Semantic),
                    session_id: Some("s1".into()),
                    importance: Some(0.75),
                    tags: vec!["manual".into()],
                    metadata: Some(json!({"team": "ops"})),
                    ttl_seconds: Some(3600),
                    lang: Some("en".into()),
                    chunking: Some(ChunkingOptions {
                        max_tokens: Some(200),
                        overlap_tokens: Some(20),
                        strategy: Some(ChunkStrategy::Fixed),
                    }),
                    decompose: Some(true),
                    ..Default::default()
                }),
        )
        .await
        .unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn import_attachment_document() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/library/attachments/sha256:abc/document",
        202,
        ACCEPTED,
    )
    .match_body(Matcher::Json(json!({
        "agent_id": "docs-bot", "document_id": "manual-v3", "tags": ["manual"], "chunking": {}
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .import_attachment_document(
            "library",
            "sha256:abc",
            ImportAttachmentDocumentRequest::new("docs-bot")
                .with_document_id("manual-v3")
                .with_options(DocumentImportOptions {
                    tags: vec!["manual".into()],
                    chunking: Some(ChunkingOptions::default()),
                    ..Default::default()
                }),
        )
        .await
        .unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn job_status_and_wait() {
    let mut server = mockito::Server::new_async().await;
    let path = "/v1/memory/documents/jobs/job_1_7";
    let query = Matcher::UrlEncoded("agent_id".into(), "docs-bot".into());
    let running = json_mock(
        &mut server,
        "GET",
        path,
        200,
        r#"{"id":"job_1_7","job_type":"document_import","status":"Running","created_at":1,
            "started_at":2,"completed_at":null,"progress":40,"message":"chunks_stored 32/80",
            "metadata":{"agent_id":"docs-bot","document_id":"manual-v3"}}"#,
    )
    .match_query(query.clone())
    .expect(2)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let job = client.document_job("docs-bot", "job_1_7").await.unwrap();
    assert_eq!(job.status, "Running");
    assert_eq!(job.progress, 40);
    assert_eq!(job.metadata["document_id"], "manual-v3");
    assert!(!job.is_finished());

    // The first wait poll still sees Running; then the job completes.
    let waiter = {
        let client = client.clone();
        tokio::spawn(async move {
            client
                .wait_for_document_job_every(
                    "docs-bot",
                    "job_1_7",
                    Duration::from_secs(10),
                    Duration::from_millis(200),
                )
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    running.assert_async().await;
    running.remove_async().await;
    let _done = json_mock(
        &mut server,
        "GET",
        path,
        200,
        r#"{"id":"job_1_7","job_type":"document_import","status":"Completed","created_at":1,
            "started_at":2,"completed_at":3,"progress":100,"message":"80 chunks","metadata":{},
            "future":1}"#,
    )
    .match_query(query)
    .create_async()
    .await;
    let done = waiter.await.unwrap().unwrap();
    assert!(done.is_completed());
    assert_eq!(done.progress, 100);
}

#[tokio::test]
async fn failed_job_carries_status_and_code() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(
        &mut server,
        "GET",
        "/v1/memory/documents/jobs/j",
        200,
        r#"{"id":"j","job_type":"document_import","status":"Failed","created_at":1,"progress":10,
            "message":"extraction timed out","metadata":{},
            "error":{"status":504,"code":"EXTRACTION_TIMEOUT"}}"#,
    )
    .match_query(Matcher::Any)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let job = client
        .wait_for_document_job("a", "j", Duration::from_secs(5))
        .await
        .unwrap();
    assert!(job.is_failed());
    let error = job.error.unwrap();
    assert_eq!(error.status, 504);
    assert_eq!(error.code, ServerErrorCode::ExtractionTimeout);
}

#[tokio::test]
async fn wait_times_out() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(&mut server, "GET", "/v1/memory/documents/jobs/j", 200,
        r#"{"id":"j","job_type":"document_import","status":"Pending","created_at":1,"progress":0,"metadata":{}}"#)
        .match_query(Matcher::Any)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let err = client
        .wait_for_document_job_every(
            "a",
            "j",
            Duration::from_millis(50),
            Duration::from_millis(10),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::Timeout), "{err:?}");
}

#[tokio::test]
async fn list_get_delete() {
    let mut server = mockito::Server::new_async().await;
    let list = json_mock(
        &mut server,
        "GET",
        "/v1/memory/documents",
        200,
        r#"{"documents":[
            {"document_id":"a-doc","title":"A","media_type":"text/markdown","chunk_count":3,
             "bytes":1200,"created_at":1,"updated_at":2,"sha256":"ab"},
            {"document_id":"b-doc","media_type":"application/pdf","chunk_count":9,"bytes":99,
             "pages":4,"created_at":1,"updated_at":1,"sha256":"cd","attachment_ref":"sha256:cd"}],
            "next_cursor":"b-doc"}"#,
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("agent_id".into(), "docs bot".into()),
        Matcher::UrlEncoded("cursor".into(), "a&b".into()),
        Matcher::UrlEncoded("limit".into(), "2".into()),
    ]))
    .create_async()
    .await;
    let get = json_mock(&mut server, "GET", "/v1/memory/documents/x%2Fy", 200,
        r#"{"document":{"document_id":"x/y","media_type":"text/plain","chunk_count":2,"bytes":10,
             "created_at":1,"updated_at":1,"sha256":"ef"},
            "chunks":[{"id":"doc_1","chunk_index":0,"heading_path":["T"],"char_start":0,"char_end":5},
                      {"id":"doc_2","chunk_index":1,"page_start":2,"page_end":3,
                       "heading_path":["T","A"],"char_start":5,"char_end":10}]}"#)
        .match_query(Matcher::UrlEncoded("agent_id".into(), "a".into()))
        .create_async()
        .await;
    let delete = json_mock(
        &mut server,
        "DELETE",
        "/v1/memory/documents/manual-v3",
        200,
        r#"{"document_id":"manual-v3","deleted_chunks":212}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "a".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();

    let page = client
        .list_documents("docs bot", Some("a&b"), Some(2))
        .await
        .unwrap();
    assert_eq!(page.documents.len(), 2);
    assert_eq!(page.next_cursor.as_deref(), Some("b-doc"));
    assert_eq!(page.documents[1].pages, Some(4));
    assert_eq!(
        page.documents[1].attachment_ref.as_deref(),
        Some("sha256:cd")
    );

    let detail = client.get_document("a", "x/y").await.unwrap();
    assert_eq!(detail.document.document_id, "x/y");
    assert_eq!(detail.chunks[1].heading_path, vec!["T", "A"]);
    assert_eq!(detail.chunks[1].page_end, Some(3));

    let deleted = client.delete_document("a", "manual-v3").await.unwrap();
    assert_eq!(deleted.deleted_chunks, 212);

    list.assert_async().await;
    get.assert_async().await;
    delete.assert_async().await;
}

#[tokio::test]
async fn old_server_404_says_why() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/documents",
        404,
        r#"{"error":"No route for POST /v1/memory/documents","code":"ROUTE_NOT_FOUND"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let err = client
        .import_document(ImportDocumentRequest::new("a", "x"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ClientError::Server { status: 404, .. }),
        "{err:?}"
    );
    assert!(err.to_string().contains("0.12.3"), "{err}");
}

#[tokio::test]
async fn feature_disabled_is_typed() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(&mut server, "POST", "/v1/memory/documents", 501,
        r#"{"error":"document import is disabled","code":"FEATURE_DISABLED","details":"DAKERA_DOCUMENTS"}"#)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let err = client
        .import_document(ImportDocumentRequest::new("a", "x"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ClientError::FeatureDisabled { .. }),
        "{err:?}"
    );
}

#[test]
fn chunk_meta_from_a_recalled_memory() {
    let r: RecallResponse = serde_json::from_str(
        r#"{"memories":[
          {"memory":{"id":"doc_9","content":"Field Manual > Install\n\nRun it.",
            "metadata":{"team":"ops","_dakera_document":{
              "id":"manual-v3","title":"Field Manual","media_type":"application/pdf",
              "chunk_index":41,"chunk_count":212,"page_start":39,"page_end":40,
              "heading_path":["Field Manual"],"char_start":80412,"char_end":81733,
              "sha256":"ab","bytes":1843311,"pages":212,"created_at":1792252596,
              "updated_at":1792252596,"fingerprint":"f","later":true}}},"score":0.9},
          {"memory":{"id":"m2","content":"plain"},"score":0.5}]}"#,
    )
    .unwrap();
    let meta = r.memories[0].document_chunk().expect("a chunk");
    assert_eq!(meta.id, "manual-v3");
    assert_eq!(meta.title.as_deref(), Some("Field Manual"));
    assert_eq!(meta.chunk_index, 41);
    assert_eq!(meta.page_start, Some(39));
    assert_eq!(meta.heading_path, vec!["Field Manual"]);
    assert!(r.memories[1].document_chunk().is_none());
}

#[test]
fn chunk_strategy_wire_names_and_unknown() {
    assert_eq!(
        serde_json::to_value(ChunkStrategy::Structure).unwrap(),
        json!("structure")
    );
    assert_eq!(
        serde_json::to_value(ChunkStrategy::Fixed).unwrap(),
        json!("fixed")
    );
    assert_eq!(
        serde_json::from_value::<ChunkStrategy>(json!("semantic")).unwrap(),
        ChunkStrategy::Unknown
    );
    assert_eq!(
        serde_json::to_value(ChunkingOptions::default()).unwrap(),
        json!({})
    );
}

#[test]
fn capabilities_documents() {
    let caps: ServerCapabilities = serde_json::from_str(
        r#"{"server_version":"0.12.3","documents":{"enabled":true,
          "text_media_types":["text/plain","text/markdown","text/html"],
          "media_types":["application/pdf"],"max_bytes":26214400,"max_text_bytes":20971520,
          "max_chunks":5000,"default_chunk_tokens":400,"default_overlap_tokens":50,
          "strategies":["structure","fixed"]}}"#,
    )
    .unwrap();
    assert!(caps.supports_documents());
    assert_eq!(caps.documents.max_bytes, 26_214_400);
    assert_eq!(caps.documents.strategies, vec!["structure", "fixed"]);
    let old: ServerCapabilities = serde_json::from_str(r#"{"server_version":"0.12.2"}"#).unwrap();
    assert!(!old.supports_documents());
}

// Recall of a document chunk is plain recall: nothing extra to send.
#[test]
fn recall_request_unchanged() {
    let v = serde_json::to_value(RecallRequest::new("a", "q")).unwrap();
    assert!(v.get("document").is_none());
}
