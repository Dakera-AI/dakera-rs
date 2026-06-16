//! Dakera Rust SDK — Playground Quickstart
//!
//! Demonstrates the 4 core memory operations against the Dakera Playground.
//!
//! Run:
//!   cargo run --example playground

use dakera_client::{DakeraClient, EdgeType, MemoryType, RecallRequest, StoreMemoryRequest};

const AGENT_ID: &str = "playground-agent";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DAKERA_API_URL")
        .unwrap_or_else(|_| "http://5.75.177.31".to_string());
    let api_key = std::env::var("DAKERA_API_KEY")
        .unwrap_or_else(|_| "playground-demo".to_string());

    let client = DakeraClient::builder(&url).api_key(&api_key).build()?;

    let health = client.health().await?;
    println!(
        "Playground: {} (healthy: {})",
        health.version.as_deref().unwrap_or("unknown"),
        health.healthy
    );

    // -------------------------------------------------------------------------
    // 1. Store memories
    // -------------------------------------------------------------------------
    println!("\n--- 1. Store Memories ---");

    let mem1 = client
        .store_memory(
            StoreMemoryRequest::new(
                AGENT_ID,
                "Dakera provides persistent, decay-weighted memory for AI agents.",
            )
            .with_type(MemoryType::Semantic)
            .with_importance(0.9)
            .with_tags(vec!["dakera".into(), "memory".into(), "overview".into()]),
        )
        .await?;
    println!("Stored: {}", mem1.memory_id);

    let mem2 = client
        .store_memory(
            StoreMemoryRequest::new(
                AGENT_ID,
                "The recall API returns semantically similar memories ranked by relevance.",
            )
            .with_type(MemoryType::Semantic)
            .with_importance(0.8)
            .with_tags(vec!["dakera".into(), "recall".into(), "api".into()]),
        )
        .await?;
    println!("Stored: {}", mem2.memory_id);

    let mem3 = client
        .store_memory(
            StoreMemoryRequest::new(
                AGENT_ID,
                "Session scoping lets agents isolate memories per task or conversation.",
            )
            .with_type(MemoryType::Episodic)
            .with_importance(0.7)
            .with_tags(vec!["sessions".into(), "isolation".into()]),
        )
        .await?;
    println!("Stored: {}", mem3.memory_id);

    // -------------------------------------------------------------------------
    // 2. Recall by query (semantic search)
    // -------------------------------------------------------------------------
    println!("\n--- 2. Recall by Query ---");

    let recalled = client
        .recall(RecallRequest::new(AGENT_ID, "How does Dakera memory work?").with_top_k(5))
        .await?;
    println!("Recalled {} memories:", recalled.memories.len());
    for m in &recalled.memories {
        let preview: String = m.content.chars().take(80).collect();
        println!("  [{:.3}] {}", m.importance, preview);
    }

    // -------------------------------------------------------------------------
    // 3. Search with filters (type=semantic)
    // -------------------------------------------------------------------------
    println!("\n--- 3. Search with Filters ---");

    let filtered = client
        .search_memories(
            RecallRequest::new(AGENT_ID, "memory API")
                .with_top_k(3)
                .with_type(MemoryType::Semantic),
        )
        .await?;
    println!("Filtered search ({} results):", filtered.memories.len());
    for m in &filtered.memories {
        let preview: String = m.content.chars().take(80).collect();
        println!("  [{:.3}] {}", m.importance, preview);
    }

    // -------------------------------------------------------------------------
    // 4. Knowledge graph link
    // -------------------------------------------------------------------------
    println!("\n--- 4. Knowledge Graph Link ---");

    let link = client
        .memory_link(&mem1.memory_id, &mem2.memory_id, EdgeType::RelatedTo)
        .await?;
    println!(
        "Linked {} → {}: edge_type={:?}",
        mem1.memory_id, mem2.memory_id, link.edge.edge_type
    );

    println!("\nPlayground quickstart complete! Visit https://dakera.ai to learn more.");
    Ok(())
}
