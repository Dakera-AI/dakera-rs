//! Vector operations — bulk upsert, bulk update/delete, count, aggregate, export
//!
//! Run: cargo run --example vector_operations

use std::collections::HashMap;

use dakera_client::{
    AggregationRequest, BulkDeleteRequest, BulkUpdateRequest, CountVectorsRequest,
    CreateNamespaceRequest, DakeraClient, ExportRequest, UpsertRequest, Vector,
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

    let namespace = "example-vectors-ops";

    // Create namespace
    client.create_namespace(namespace, CreateNamespaceRequest::new().with_dimensions(4)).await?;

    // =========================================================================
    // Bulk Upsert
    // =========================================================================

    println!("\n--- Bulk Upsert ---");
    let vectors: Vec<Vector> = (0..20)
        .map(|i| {
            Vector::with_metadata(
                &format!("vec-{:03}", i),
                vec![i as f32 * 0.05, 0.1, 0.2, 0.3 + i as f32 * 0.01],
                HashMap::from([
                    ("category".into(), serde_json::json!(if i % 2 == 0 { "even" } else { "odd" })),
                    ("score".into(), serde_json::json!(i as f64 * 1.5)),
                    ("batch".into(), serde_json::json!(i / 10)),
                ]),
            )
        })
        .collect();

    let upsert_resp = client
        .upsert(namespace, UpsertRequest { vectors })
        .await?;
    println!("Upserted {} vectors", upsert_resp.upserted_count);
    assert_eq!(upsert_resp.upserted_count, 20, "expected 20 vectors upserted");

    // =========================================================================
    // Count Vectors
    // =========================================================================

    println!("\n--- Count Vectors ---");
    let count_all = client
        .count_vectors(namespace, CountVectorsRequest { filter: None })
        .await?;
    println!("Total vectors in '{}': {}", count_all.namespace, count_all.count);
    assert!(
        count_all.count >= 20,
        "expected at least 20 vectors"
    );

    // Count with filter
    let count_even = client
        .count_vectors(
            namespace,
            CountVectorsRequest {
                filter: Some(serde_json::json!({ "category": { "$eq": "even" } })),
            },
        )
        .await?;
    println!("Even vectors: {}", count_even.count);
    assert_eq!(count_even.count, 10, "expected 10 even vectors");

    // =========================================================================
    // Bulk Update
    // =========================================================================

    println!("\n--- Bulk Update ---");
    let update_resp = client
        .bulk_update_vectors(
            namespace,
            BulkUpdateRequest {
                filter: serde_json::json!({ "category": { "$eq": "even" } }),
                update: serde_json::json!({ "status": "promoted", "priority": 1 }),
            },
        )
        .await?;
    println!(
        "Updated {} vectors, failed: {}",
        update_resp.updated, update_resp.failed
    );
    assert!(
        update_resp.updated > 0,
        "expected at least one vector updated"
    );

    // =========================================================================
    // Aggregation
    // =========================================================================

    println!("\n--- Aggregation ---");
    let agg_resp = client
        .aggregate(
            namespace,
            AggregationRequest::new()
                .with_count("total_count")
                .with_sum("total_score", "score")
                .with_avg("avg_score", "score")
                .with_min("min_score", "score")
                .with_max("max_score", "score")
                .with_group_by("category"),
        )
        .await?;
    println!("Aggregation results:");
    if let Some(groups) = &agg_resp.aggregation_groups {
        for group in groups {
            println!(
                "  Group {:?}: {:?}",
                group.group_key, group.aggregations
            );
        }
        assert!(
            !groups.is_empty(),
            "expected non-empty aggregation groups"
        );
    }

    // =========================================================================
    // Export Vectors
    // =========================================================================

    println!("\n--- Export Vectors ---");
    let export_resp = client
        .export_vectors(
            namespace,
            ExportRequest::new().with_top_k(5).include_metadata(true),
        )
        .await?;
    println!(
        "Exported {} vectors (has more: {})",
        export_resp.vectors.len(),
        export_resp.next_cursor.is_some()
    );
    for v in &export_resp.vectors {
        let dims = v.values.as_ref().map_or(0, |vals| vals.len());
        println!("  {} — dims: {}", v.id, dims);
    }
    assert!(
        !export_resp.vectors.is_empty(),
        "expected non-empty export"
    );

    // =========================================================================
    // Bulk Delete
    // =========================================================================

    println!("\n--- Bulk Delete ---");
    let del_resp = client
        .bulk_delete_vectors(
            namespace,
            BulkDeleteRequest {
                filter: serde_json::json!({ "category": { "$eq": "odd" } }),
            },
        )
        .await?;
    println!(
        "Deleted {} vectors, failed: {}",
        del_resp.deleted, del_resp.failed
    );
    assert!(
        del_resp.deleted > 0,
        "expected at least one vector deleted"
    );

    // Verify remaining count
    let count_after = client
        .count_vectors(namespace, CountVectorsRequest { filter: None })
        .await?;
    println!("Remaining vectors: {}", count_after.count);

    println!("\nVector operations example completed successfully.");
    Ok(())
}
