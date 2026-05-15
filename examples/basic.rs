//! Basic Dakera Rust SDK usage — vectors, namespaces, search
//!
//! Run: cargo run --example basic

use dakera_client::{CreateNamespaceRequest, DakeraClient, QueryRequest, UpsertRequest, Vector};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DakeraClient::builder("http://localhost:3300")
        .api_key("dk-mykey")
        .build()?;

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
        .create_namespace(
            namespace,
            CreateNamespaceRequest {
                dimension: Some(3),
                distance_metric: None,
            },
        )
        .await?;

    // Upsert vectors with metadata
    let resp = client
        .upsert(
            namespace,
            UpsertRequest {
                vectors: vec![
                    Vector {
                        id: "vec1".to_string(),
                        values: vec![0.1, 0.2, 0.3],
                        metadata: Some(serde_json::json!({
                            "category": "electronics",
                            "price": 299.99
                        })),
                    },
                    Vector {
                        id: "vec2".to_string(),
                        values: vec![0.4, 0.5, 0.6],
                        metadata: Some(serde_json::json!({
                            "category": "books",
                            "price": 19.99
                        })),
                    },
                    Vector {
                        id: "vec3".to_string(),
                        values: vec![0.15, 0.25, 0.35],
                        metadata: Some(serde_json::json!({
                            "category": "electronics",
                            "price": 599.99
                        })),
                    },
                ],
            },
        )
        .await?;
    println!("Upserted {} vectors", resp.upserted_count);

    // Query similar vectors
    println!("\n--- Query Results ---");
    let results = client
        .query(
            namespace,
            QueryRequest {
                vector: vec![0.1, 0.2, 0.3],
                top_k: 10,
                include_metadata: true,
                ..Default::default()
            },
        )
        .await?;
    for m in &results.matches {
        println!("ID: {}, Score: {:.4}", m.id, m.score);
    }

    // Query with metadata filter
    println!("\n--- Filtered Query (electronics only) ---");
    let filtered = client
        .query(
            namespace,
            QueryRequest {
                vector: vec![0.1, 0.2, 0.3],
                top_k: 10,
                filter: Some(serde_json::json!({
                    "category": { "$eq": "electronics" }
                })),
                include_metadata: true,
                ..Default::default()
            },
        )
        .await?;
    for m in &filtered.matches {
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
