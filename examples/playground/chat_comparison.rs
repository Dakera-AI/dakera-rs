//! LLM Chat Comparison — with and without Dakera memory
//!
//! Demonstrates the pattern used by the Dakera playground: run the same user
//! query through two paths and compare responses.
//!
//!   Path A (memory-augmented) — recall relevant context, prepend to prompt
//!   Path B (baseline)         — send the raw prompt with no memory context
//!
//! Run:
//!   DAKERA_API_URL=https://5-75-177-31.sslip.io DAKERA_API_KEY=<key> \
//!     cargo run --example chat_comparison

use std::sync::Arc;

use dakera_client::{ChatMemorySession, DakeraClient, RecalledMemory};

const AGENT_ID: &str = "playground-demo";
const DEFAULT_URL: &str = "https://5-75-177-31.sslip.io";
const DEFAULT_KEY: &str = "playground-demo";

fn build_context_prompt(memories: &[RecalledMemory], user_message: &str) -> String {
    if memories.is_empty() {
        return user_message.to_owned();
    }
    let context_lines: Vec<String> = memories.iter().map(|m| format!("- {}", m.content)).collect();
    format!(
        "[Relevant context from memory]\n{}\n\n[User message]\n{}",
        context_lines.join("\n"),
        user_message
    )
}

fn call_llm(prompt: &str) -> &'static str {
    // Placeholder — swap in any LLM provider:
    //
    //   let openai = async_openai::Client::new();
    //   let req = CreateChatCompletionRequestArgs::default()
    //       .model("gpt-4o-mini")
    //       .messages([ChatCompletionRequestUserMessageArgs::default()
    //           .content(prompt).build()?])
    //       .build()?;
    //   let resp = openai.chat().create(req).await?;
    //   return resp.choices[0].message.content.clone().unwrap_or_default();
    if prompt.contains("[Relevant context from memory]") {
        "I recall you mentioned this before. Here is a context-aware answer."
    } else {
        "I have no prior context. Here is a generic answer."
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DAKERA_API_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
    let api_key = std::env::var("DAKERA_API_KEY").unwrap_or_else(|_| DEFAULT_KEY.to_string());

    let client = Arc::new(DakeraClient::builder(&url).api_key(&api_key).build()?);

    println!("=== Dakera Playground — LLM Chat Comparison Demo ===\n");

    // ------------------------------------------------------------------
    // Step 1: Seed some prior conversation turns
    // ------------------------------------------------------------------
    println!("Seeding prior conversation turns into Dakera memory...");
    let seed = ChatMemorySession::create_with_metadata(
        Arc::clone(&client),
        AGENT_ID,
        serde_json::json!({"source": "playground-seed"}),
    )
    .await?;
    seed.store("user", "I'm building a chatbot in Rust using async Tokio.").await?;
    seed.store("assistant", "Great choice — Tokio is the async foundation for Rust services.").await?;
    seed.store("user", "My team prefers type-safe APIs so we use Axum on the backend.").await?;
    println!("  Session {}: stored 3 turns\n", seed.session_id());
    seed.close().await?;

    // ------------------------------------------------------------------
    // Step 2: Start a new session and compare responses
    // ------------------------------------------------------------------
    let follow_up = "What framework should I use for the async background tasks?";

    let session = ChatMemorySession::create_with_metadata(
        Arc::clone(&client),
        AGENT_ID,
        serde_json::json!({"source": "playground-compare"}),
    )
    .await?;
    println!("Comparison session: {}", session.session_id());
    println!("User: {}\n", follow_up);

    // Path A — memory-augmented
    let memories = session.recall_top_k(follow_up, 5).await?;
    let augmented_prompt = build_context_prompt(&memories, follow_up);
    let response_with_memory = call_llm(&augmented_prompt);

    // Path B — baseline (no memory)
    let response_without_memory = call_llm(follow_up);

    // Store the actual exchange
    session.store("user", follow_up).await?;
    session.store("assistant", response_with_memory).await?;
    session.close().await?;

    // ------------------------------------------------------------------
    // Step 3: Print side-by-side comparison
    // ------------------------------------------------------------------
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│  WITHOUT Dakera memory                                      │");
    println!("├─────────────────────────────────────────────────────────────┤");
    println!("│  {response_without_memory}");
    println!("├─────────────────────────────────────────────────────────────┤");
    println!("│  WITH Dakera memory                                         │");
    println!("├─────────────────────────────────────────────────────────────┤");
    println!("│  {response_with_memory}");
    println!("└─────────────────────────────────────────────────────────────┘");

    if !memories.is_empty() {
        println!("\n  Memory used: {} relevant context item(s)", memories.len());
        for m in &memories {
            let preview: String = m.content.chars().take(80).collect();
            println!("    • [{:.2}] {}", m.score, preview);
        }
    }

    Ok(())
}
