//! Playground scenario integration tests for the Dakera Rust SDK.
//!
//! Validates the core store→recall→search→KG-link workflow that the playground
//! quickstart demonstrates. Tests are skipped unless DAKERA_TEST_URL is set.
//!
//! Run:
//!   DAKERA_TEST_URL=http://localhost:3000 DAKERA_API_KEY=test-key \
//!     cargo test --test playground_integration_test

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use dakera_client::{DakeraClient, EdgeType, MemoryType, RecallRequest, StoreMemoryRequest};

fn get_client() -> Option<DakeraClient> {
    let url = env::var("DAKERA_TEST_URL").ok()?;
    let api_key = env::var("DAKERA_API_KEY").unwrap_or_else(|_| "test-key".to_string());
    Some(
        DakeraClient::builder(&url)
            .api_key(&api_key)
            .build()
            .expect("Failed to create client"),
    )
}

fn unique_agent() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    format!("playground-integ-{:08x}", nanos)
}

/// End-to-end playground scenario: store → recall → search → KG link.
///
/// Runs as a single async test so the memory IDs produced in step 1 are
/// available for steps 2–4 without sharing state across test functions.
#[tokio::test]
async fn test_playground_workflow() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping playground integration test");
        return;
    };

    let agent_id = unique_agent();

    // ------------------------------------------------------------------
    // Step 1: store two memories with tags
    // ------------------------------------------------------------------
    let mem1 = client
        .store_memory(
            StoreMemoryRequest::new(
                &agent_id,
                "Dakera provides persistent, decay-weighted memory for AI agents.",
            )
            .with_type(MemoryType::Semantic)
            .with_importance(0.9)
            .with_tags(vec!["dakera".into(), "memory".into(), "overview".into()]),
        )
        .await
        .expect("step 1a: store_memory must succeed");
    assert!(!mem1.memory_id.is_empty(), "mem1 must have a non-empty ID");

    let mem2 = client
        .store_memory(
            StoreMemoryRequest::new(
                &agent_id,
                "The recall API returns semantically similar memories ranked by relevance.",
            )
            .with_type(MemoryType::Semantic)
            .with_importance(0.8)
            .with_tags(vec!["dakera".into(), "recall".into(), "api".into()]),
        )
        .await
        .expect("step 1b: store_memory must succeed");
    assert!(!mem2.memory_id.is_empty(), "mem2 must have a non-empty ID");

    // ------------------------------------------------------------------
    // Step 2: recall by semantic query
    // ------------------------------------------------------------------
    let recalled = client
        .recall(RecallRequest::new(&agent_id, "How does Dakera memory work?").with_top_k(5))
        .await
        .expect("step 2: recall must succeed");
    assert!(
        !recalled.memories.is_empty(),
        "recall must return at least one memory"
    );
    for m in &recalled.memories {
        assert!(
            !m.content.is_empty(),
            "each recalled memory must have content"
        );
    }

    // ------------------------------------------------------------------
    // Step 3: search memories with memory_type filter
    // ------------------------------------------------------------------
    let filtered = client
        .search_memories(
            RecallRequest::new(&agent_id, "memory API")
                .with_top_k(5)
                .with_type(MemoryType::Semantic),
        )
        .await
        .expect("step 3: search_memories must succeed");
    assert!(
        !filtered.memories.is_empty(),
        "filtered search must return at least one result"
    );
    for m in &filtered.memories {
        assert!(
            !m.content.is_empty(),
            "each search result must have content"
        );
    }

    // ------------------------------------------------------------------
    // Step 4: knowledge graph link
    // ------------------------------------------------------------------
    let link = client
        .memory_link(&mem1.memory_id, &mem2.memory_id, EdgeType::RelatedTo)
        .await
        .expect("step 4: memory_link must succeed");
    assert_eq!(
        link.edge.edge_type,
        EdgeType::RelatedTo,
        "KG edge must be RelatedTo"
    );
}
