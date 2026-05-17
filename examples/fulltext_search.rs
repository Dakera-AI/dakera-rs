//! Full-text search — index, search, stats, delete
//!
//! Run: cargo run --example fulltext_search

use std::collections::HashMap;

use dakera_client::{
    CreateNamespaceRequest, DakeraClient, DeleteRequest, Document, FullTextSearchRequest,
    IndexDocumentsRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url =
        std::env::var("DAKERA_API_URL").unwrap_or_else(|_| "http://localhost:3300".to_string());
    let api_key = std::env::var("DAKERA_API_KEY").unwrap_or_else(|_| "dk-mykey".to_string());
    let client = DakeraClient::builder(&url).api_key(&api_key).build()?;

    let health = client.health().await?;
    println!(
        "Server: {} (healthy: {})",
        health.version.as_deref().unwrap_or("unknown"),
        health.healthy
    );

    let namespace = "example-fulltext";

    // Create namespace for full-text indexing
    client.create_namespace(namespace, CreateNamespaceRequest::new().with_dimensions(3)).await?;

    // =========================================================================
    // Index Documents
    // =========================================================================

    println!("\n--- Indexing Documents ---");
    let docs = vec![
        Document::with_metadata(
            "doc1",
            "Rust is a systems programming language focused on safety and performance",
            HashMap::from([("category".into(), serde_json::json!("programming"))]),
        ),
        Document::with_metadata(
            "doc2",
            "Python is widely used for machine learning and data science applications",
            HashMap::from([("category".into(), serde_json::json!("programming"))]),
        ),
        Document::with_metadata(
            "doc3",
            "Vector databases enable efficient similarity search for AI applications",
            HashMap::from([("category".into(), serde_json::json!("databases"))]),
        ),
        Document::new(
            "doc4",
            "Knowledge graphs represent relationships between entities in structured form",
        ),
        Document::new(
            "doc5",
            "Agent memory systems help AI assistants maintain context across conversations",
        ),
    ];

    let index_resp = client
        .index_documents(namespace, IndexDocumentsRequest { documents: docs })
        .await?;
    println!("Indexed {} documents", index_resp.indexed_count);
    assert!(
        index_resp.indexed_count >= 5,
        "expected at least 5 documents indexed"
    );

    // =========================================================================
    // Full-Text Search
    // =========================================================================

    println!("\n--- Search: 'programming language' ---");
    let results = client
        .fulltext_search(namespace, FullTextSearchRequest::new("programming language", 10))
        .await?;
    for m in &results.results {
        let text = m.text.as_deref().unwrap_or("");
        println!("  ID: {}, Score: {:.4}, Text: {:.60}", m.id, m.score, text);
    }
    assert!(
        !results.results.is_empty(),
        "expected non-empty search results"
    );

    // Search with filter
    println!("\n--- Search: 'AI' (databases only) ---");
    let filtered = client
        .fulltext_search(
            namespace,
            FullTextSearchRequest::new("AI", 10).with_filter(serde_json::json!({
                "category": { "$eq": "databases" }
            })),
        )
        .await?;
    for m in &filtered.results {
        println!("  ID: {}, Score: {:.4}", m.id, m.score);
    }

    // Simple convenience search
    println!("\n--- Simple Search: 'memory' ---");
    let simple = client.search_text(namespace, "memory", 5).await?;
    println!("Found {} results for 'memory'", simple.results.len());

    // =========================================================================
    // Full-Text Stats
    // =========================================================================

    println!("\n--- Full-Text Index Stats ---");
    let stats = client.fulltext_stats(namespace).await?;
    println!(
        "Documents: {}, Terms: {}",
        stats.document_count, stats.term_count
    );
    assert!(
        stats.document_count >= 5,
        "expected at least 5 documents in index"
    );

    // =========================================================================
    // Delete from Full-Text Index
    // =========================================================================

    println!("\n--- Deleting Documents ---");
    let del_resp = client
        .fulltext_delete(namespace, DeleteRequest::single("doc4"))
        .await?;
    println!("Deleted {} documents from full-text index", del_resp.deleted_count);

    // Verify deletion
    let stats_after = client.fulltext_stats(namespace).await?;
    println!(
        "Documents after deletion: {} (was {})",
        stats_after.document_count, stats.document_count
    );

    println!("\nFull-text search example completed successfully.");
    Ok(())
}
