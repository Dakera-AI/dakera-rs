//! Admin operations — backup, cluster, maintenance, quota
//!
//! Run: cargo run --example admin_operations

use dakera_client::{CreateBackupRequest, DakeraClient, QuotaConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url =
        std::env::var("DAKERA_API_URL").unwrap_or_else(|_| "http://localhost:3300".to_string());
    let api_key = std::env::var("DAKERA_API_KEY").unwrap_or_else(|_| "dk-mykey".to_string());
    let client = DakeraClient::builder(&url).api_key(&api_key).build()?;

    // Check server health
    let health = client.health().await?;
    println!(
        "Server: {} (healthy: {})",
        health.version.as_deref().unwrap_or("unknown"),
        health.healthy
    );
    assert!(
        health.healthy,
        "server must be healthy"
    );

    // =========================================================================
    // Cluster Status
    // =========================================================================

    println!("\n--- Cluster Status ---");
    let cluster = client.cluster_status().await?;
    println!(
        "Cluster: {} | State: {} | Nodes: {} | Vectors: {}",
        cluster.cluster_id, cluster.state, cluster.node_count, cluster.total_vectors
    );
    assert!(
        !cluster.cluster_id.is_empty(),
        "expected non-empty cluster ID"
    );

    // List cluster nodes
    let nodes = client.cluster_nodes().await?;
    println!("Nodes in cluster: {}", nodes.total);
    for node in &nodes.nodes {
        println!(
            "  {} ({}) — {} vectors, uptime {}s",
            node.node_id, node.role, node.vector_count, node.uptime_seconds
        );
    }
    assert!(
        !nodes.nodes.is_empty(),
        "expected at least one node"
    );

    // =========================================================================
    // Maintenance Mode
    // =========================================================================

    println!("\n--- Maintenance Status ---");
    let maint = client.admin_maintenance_status().await?;
    println!("Maintenance enabled: {}", maint.enabled);

    // =========================================================================
    // Backups
    // =========================================================================

    println!("\n--- Backups ---");
    let backup_resp = client
        .create_backup(CreateBackupRequest {
            name: "example-backup".to_string(),
            backup_type: Some("full".to_string()),
            namespaces: None,
            encrypt: Some(false),
            compression: Some("gzip".to_string()),
        })
        .await?;
    println!(
        "Created backup: {} (status: {})",
        backup_resp.backup.backup_id, backup_resp.backup.status
    );
    assert!(
        !backup_resp.backup.backup_id.is_empty(),
        "expected non-empty backup ID"
    );

    // List backups
    let backups = client.list_backups().await?;
    println!("Total backups: {}", backups.total);
    for b in &backups.backups {
        println!(
            "  {} — {} ({} vectors, {} bytes)",
            b.backup_id, b.status, b.vector_count, b.size_bytes
        );
    }
    assert!(
        !backups.backups.is_empty(),
        "expected at least one backup"
    );

    // Clean up the backup we just created
    client.delete_backup(&backup_resp.backup.backup_id).await?;
    println!("Deleted backup: {}", backup_resp.backup.backup_id);

    // =========================================================================
    // Quotas
    // =========================================================================

    println!("\n--- Quotas ---");
    let quota_config = QuotaConfig {
        max_vectors: Some(100_000),
        max_storage_bytes: Some(1_073_741_824), // 1 GiB
        max_queries_per_minute: Some(1000),
        max_writes_per_minute: Some(500),
    };
    client.set_quota("example-quota-ns", quota_config).await?;
    println!("Set quota for namespace 'example-quota-ns'");

    let quota = client.get_quota("example-quota-ns").await?;
    println!(
        "Quota — max vectors: {:?}, max storage: {:?} bytes",
        quota.config.max_vectors, quota.config.max_storage_bytes
    );
    assert_eq!(
        quota.config.max_vectors,
        Some(100_000),
        "expected max_vectors = 100000"
    );

    // Clean up
    client.delete_quota("example-quota-ns").await?;
    println!("Deleted quota for 'example-quota-ns'");

    // =========================================================================
    // Runtime Configuration
    // =========================================================================

    println!("\n--- Runtime Config ---");
    let config = client.get_config().await?;
    println!(
        "Cache enabled: {}, Rate limit: {} rps, Query timeout: {} ms",
        config.cache_enabled, config.rate_limit_rps, config.query_timeout_ms
    );

    println!("\nAll admin operations completed successfully.");
    Ok(())
}
