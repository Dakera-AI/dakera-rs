//! Dakera Rust SDK — Advanced Features
//!
//! Covers: text auto-embedding, full-text search, hybrid search, filters
//!
//! Run: cargo run --example advanced

use dakera_client::{
    filter, DakeraClient, Document, HybridSearchRequest, IndexDocumentsRequest, UpsertTextRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url =
        std::env::var("DAKERA_API_URL").unwrap_or_else(|_| "http://localhost:3300".to_string());
    let api_key = std::env::var("DAKERA_API_KEY").unwrap_or_else(|_| "dk-mykey".to_string());
    let client = DakeraClient::builder(&url).api_key(&api_key).build()?;

    let namespace = "example-advanced";

    // -------------------------------------------------------------------------
    // Text auto-embedding (server generates vectors)
    // -------------------------------------------------------------------------
    println!("--- Text Auto-Embedding ---");

    let text_resp = client
        .upsert_text(
            namespace,
            UpsertTextRequest::new(vec![
                dakera_client::TextDocument::new(
                    "doc1",
                    "Rust memory safety prevents data races at compile time.",
                ),
                dakera_client::TextDocument::new(
                    "doc2",
                    "Go goroutines enable lightweight concurrency patterns.",
                ),
                dakera_client::TextDocument::new(
                    "doc3",
                    "Python asyncio provides cooperative multitasking.",
                ),
            ]),
        )
        .await?;
    println!("Upserted {} text documents", text_resp.upserted_count);

    let text_results = client
        .query_text_simple(namespace, "concurrent programming", 3)
        .await?;
    println!("Text search results:");
    for r in &text_results.results {
        println!("  {}: score {:.4}", r.id, r.score);
    }

    // -------------------------------------------------------------------------
    // Full-text search (BM25)
    // -------------------------------------------------------------------------
    println!("\n--- Full-Text Search ---");

    client
        .index_documents(
            namespace,
            IndexDocumentsRequest {
                documents: vec![
                    Document {
                        id: "ft1".to_string(),
                        text: "Vector databases enable semantic search over embeddings."
                            .to_string(),
                        metadata: None,
                    },
                    Document {
                        id: "ft2".to_string(),
                        text: "BM25 ranking uses term frequency and document length.".to_string(),
                        metadata: None,
                    },
                ],
            },
        )
        .await?;

    let ft_results = client.search_text(namespace, "vector search", 5).await?;
    println!("Full-text results:");
    for r in &ft_results.results {
        println!("  {}: score {:.4}", r.id, r.score);
    }

    // -------------------------------------------------------------------------
    // Hybrid search (vector + BM25)
    // -------------------------------------------------------------------------
    println!("\n--- Hybrid Search ---");

    let hybrid_results = client
        .hybrid_search(
            namespace,
            HybridSearchRequest::text_only("semantic search", 5),
        )
        .await?;
    println!("Hybrid results:");
    for r in &hybrid_results.results {
        println!("  {}: score {:.4}", r.id, r.score);
    }

    // -------------------------------------------------------------------------
    // Filter DSL
    // -------------------------------------------------------------------------
    println!("\n--- Typed Filter DSL ---");

    let _filter = serde_json::json!({
        "category": filter::eq("electronics"),
        "price": filter::gte(100.0),
    });
    println!("Filter: {}", _filter);

    println!("\nDone!");
    Ok(())
}
