//! Dakera Rust SDK — Memory & Session Operations
//!
//! Run: cargo run --example memory

use dakera_client::{DakeraClient, MemoryType, RecallRequest, StoreMemoryRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DAKERA_API_URL").unwrap_or_else(|_| "http://localhost:3300".to_string());
    let client = DakeraClient::builder(&url)
        .api_key("dk-mykey")
        .build()?;

    let agent_id = "agent-demo";

    // -------------------------------------------------------------------------
    // Store memories
    // -------------------------------------------------------------------------
    println!("--- Storing Memories ---");

    let mem1 = client
        .store_memory(StoreMemoryRequest {
            agent_id: agent_id.to_string(),
            content: "The user prefers concise responses with code examples.".to_string(),
            memory_type: MemoryType::Semantic,
            importance: 0.9,
            metadata: Some(serde_json::json!({ "source": "user-feedback" })),
            tags: Vec::new(),
            session_id: None,
            ttl_seconds: None,
            expires_at: None,
        })
        .await?;
    println!("Stored memory: {}", mem1.memory_id);

    let mem2 = client
        .store_memory(
            StoreMemoryRequest::new(agent_id, "User is building a Rust web service with Axum.")
                .with_type(MemoryType::Episodic)
                .with_importance(0.7),
        )
        .await?;
    println!("Stored memory: {}", mem2.memory_id);

    // -------------------------------------------------------------------------
    // Recall memories (semantic search)
    // -------------------------------------------------------------------------
    println!("\n--- Recalling Memories ---");

    let recalled = client
        .recall(RecallRequest::new(agent_id, "What does the user prefer?").with_top_k(5))
        .await?;
    for m in &recalled.memories {
        println!(
            "  [{:.2}] {:?} — {}",
            m.importance, m.memory_type, m.content
        );
    }

    // -------------------------------------------------------------------------
    // Sessions
    // -------------------------------------------------------------------------
    println!("\n--- Session Management ---");

    let session = client.start_session(agent_id).await?;
    println!("Started session: {}", session.id);

    client
        .store_memory(
            StoreMemoryRequest::new(
                agent_id,
                "Reviewing PR #42: refactor authentication middleware.",
            )
            .with_session(session.id.clone()),
        )
        .await?;
    println!("Stored session-scoped memory");

    let end_resp = client
        .end_session(&session.id, Some("code review complete".to_string()))
        .await?;
    println!("Ended session (memories: {})", end_resp.memory_count);

    // -------------------------------------------------------------------------
    // Agent stats
    // -------------------------------------------------------------------------
    println!("\n--- Agent Stats ---");

    let stats = client.agent_stats(agent_id).await?;
    println!("Agent: {}", stats.agent_id);
    println!("  Total memories: {}", stats.total_memories);
    println!("  Total sessions: {}", stats.total_sessions);

    println!("\nDone!");
    Ok(())
}
