//! Dakera Rust SDK — Memory & Session Operations
//!
//! Run: cargo run --example memory

use dakera_client::{DakeraClient, StoreMemoryRequest, RecallRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DakeraClient::builder("http://localhost:3300")
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
            memory_type: Some("semantic".to_string()),
            importance: Some(0.9),
            metadata: Some(serde_json::json!({ "source": "user-feedback" })),
            ..Default::default()
        })
        .await?;
    println!("Stored memory: {}", mem1.memory_id);

    let mem2 = client
        .store_memory(StoreMemoryRequest {
            agent_id: agent_id.to_string(),
            content: "User is building a Rust web service with Axum.".to_string(),
            memory_type: Some("episodic".to_string()),
            importance: Some(0.7),
            ..Default::default()
        })
        .await?;
    println!("Stored memory: {}", mem2.memory_id);

    // -------------------------------------------------------------------------
    // Recall memories (semantic search)
    // -------------------------------------------------------------------------
    println!("\n--- Recalling Memories ---");

    let recalled = client
        .recall(RecallRequest {
            agent_id: agent_id.to_string(),
            query: "What does the user prefer?".to_string(),
            top_k: Some(5),
            ..Default::default()
        })
        .await?;
    for m in &recalled.memories {
        println!(
            "  [{:.2}] {} — {}",
            m.importance,
            m.memory_type.as_deref().unwrap_or("unknown"),
            m.content
        );
    }

    // -------------------------------------------------------------------------
    // Sessions
    // -------------------------------------------------------------------------
    println!("\n--- Session Management ---");

    let session = client.start_session(agent_id).await?;
    println!("Started session: {}", session.id);

    // Store a session-scoped memory
    client
        .store_memory(StoreMemoryRequest {
            agent_id: agent_id.to_string(),
            content: "Reviewing PR #42: refactor authentication middleware.".to_string(),
            session_id: Some(session.id.clone()),
            ..Default::default()
        })
        .await?;
    println!("Stored session-scoped memory");

    // End the session
    let end_resp = client.end_session(&session.id, Some("code review complete")).await?;
    println!("Ended session (status: {:?})", end_resp.status);

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
