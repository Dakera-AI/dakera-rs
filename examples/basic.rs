//! Basic Dakera Rust SDK usage — vectors, namespaces, search
//!
//! Run: cargo run --example basic

use std::collections::HashMap;

use dakera_client::{CreateNamespaceRequest, DakeraClient, QueryRequest, UpsertRequest, Vector};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url =
        std::env::var("DAKERA_API_URL").unwrap_or_else(|_| "http://localhost:3300".to_string());
    let api_key =
        std::env::var("DAKERA_API_KEY").unwrap_or_else(|_| "dk-mykey".to_string());
    let client = DakeraClient::builder(&url).api_key(&api_key).build()?;

    // Check server health
    let health = client.health().await?;
    println!(
        "Server: {} (healthy: {})",
        health.version.as_deref().unwrap_or("unknown"),
        health.healthy
    );

    let namespace = "example-vectors";

    // Create namespace
    client
        .create_namespace(namespace, CreateNamespaceRequest::new().with_dimensions(3))
        .await?;

    // Upsert vectors with metadata
    let resp = client
        .upsert(
            namespace,
            UpsertRequest {
                vectors: vec![
                    Vector::with_metadata(
                        "vec1",
                        vec![0.1, 0.2, 0.3],
                        HashMap::from([
                            ("category".into(), serde_json::json!("electronics")),
                            ("price".into(), serde_json::json!(299.99)),
                        ]),
                    ),
                    Vector::with_metadata(
                        "vec2",
                        vec![0.4, 0.5, 0.6],
                        HashMap::from([
                            ("category".into(), serde_json::json!("books")),
                            ("price".into(), serde_json::json!(19.99)),
                        ]),
                    ),
                    Vector::with_metadata(
                        "vec3",
                        vec![0.15, 0.25, 0.35],
                        HashMap::from([
                            ("category".into(), serde_json::json!("electronics")),
                            ("price".into(), serde_json::json!(599.99)),
                        ]),
                    ),
                ],
            },
        )
        .await?;
    println!("Upserted {} vectors", resp.upserted_count);

    // Query similar vectors
    println!("\n--- Query Results ---");
    let results = client
        .query(namespace, QueryRequest::new(vec![0.1, 0.2, 0.3], 10))
        .await?;
    for m in &results.results {
        println!("ID: {}, Score: {:.4}", m.id, m.score);
    }

    // Query with metadata filter
    println!("\n--- Filtered Query (electronics only) ---");
    let filtered = client
        .query(
            namespace,
            QueryRequest::new(vec![0.1, 0.2, 0.3], 10).with_filter(serde_json::json!({
                "category": { "$eq": "electronics" }
            })),
        )
        .await?;
    for m in &filtered.results {
        println!("ID: {}, Score: {:.4}", m.id, m.score);
    }

    // Fetch vectors by ID
    println!("\n--- Fetched Vectors ---");
    let vectors = client.fetch_by_ids(namespace, &["vec1", "vec2"]).await?;
    for v in &vectors {
        println!("ID: {}, Values: {:?}", v.id, v.values);
    }

    // Delete and cleanup
    client.delete_one(namespace, "vec1").await?;
    println!("\nDeleted vec1");

    Ok(())
}
