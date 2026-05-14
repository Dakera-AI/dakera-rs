//! Dakera Rust SDK — Advanced Features
//!
//! Covers: text auto-embedding, full-text search, hybrid search, filters
//!
//! Run: cargo run --example advanced

use dakera_client::{
    filter, DakeraClient, Document, HybridSearchRequest, IndexDocumentsRequest,
    QueryTextRequest, UpsertTextRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DakeraClient::builder("http://localhost:3300")
        .api_key("dk-mykey")
        .build()?;

    let namespace = "example-advanced";

    // -------------------------------------------------------------------------
    // Text auto-embedding (server generates vectors)
    // -------------------------------------------------------------------------
    println!("--- Text Auto-Embedding ---");

    let text_resp = client
        .upsert_text(
            namespace,
            UpsertTextRequest {
                documents: vec![
                    dakera_client::TextDocument {
                        id: "doc1".to_string(),
                        text: "Rust memory safety prevents data races at compile time.".to_string(),
                        metadata: None,
                    },
                    dakera_client::TextDocument {
                        id: "doc2".to_string(),
                        text: "Go goroutines enable lightweight concurrency patterns.".to_string(),
                        metadata: None,
                    },
                    dakera_client::TextDocument {
                        id: "doc3".to_string(),
                        text: "Python asyncio provides cooperative multitasking.".to_string(),
                        metadata: None,
                    },
                ],
                embedding_model: None,
            },
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
                        text: "Vector databases enable semantic search over embeddings.".to_string(),
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
            HybridSearchRequest {
                text: "semantic search".to_string(),
                vector: None,
                top_k: Some(5),
                filter: None,
                vector_weight: None,
            },
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

    // The filter module provides typed helpers instead of raw JSON
    let _filter = serde_json::json!({
        "category": filter::eq("electronics"),
        "price": filter::gte(100.0),
    });
    println!("Filter: {}", _filter);

    println!("\nDone!");
    Ok(())
}
