//! Ops diagnostics — diagnostics, jobs, compaction, cache
//!
//! Run: cargo run --example ops_diagnostics

use dakera_client::{CompactionRequest, DakeraClient};

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
    assert!(health.healthy, "server must be healthy");

    // =========================================================================
    // System Diagnostics
    // =========================================================================

    println!("\n--- System Diagnostics ---");
    let diag = client.diagnostics().await?;
    println!(
        "Version: {}, Uptime: {}s, Memory: {} bytes, Threads: {}",
        diag.system.version,
        diag.system.uptime_seconds,
        diag.resources.memory_bytes,
        diag.resources.thread_count
    );
    println!(
        "Components — storage: {}, search: {}, cache: {}",
        diag.components.storage.healthy,
        diag.components.search_engine.healthy,
        diag.components.cache.healthy
    );
    assert!(diag.resources.memory_bytes > 0, "expected non-zero memory usage");

    // =========================================================================
    // Background Jobs
    // =========================================================================

    println!("\n--- Background Jobs ---");
    let jobs = client.list_jobs().await?;
    println!("Active jobs: {}", jobs.len());
    for job in &jobs {
        println!(
            "  {} — type: {}, status: {}, progress: {}%",
            job.id, job.job_type, job.status, job.progress
        );
    }

    // =========================================================================
    // Compaction
    // =========================================================================

    println!("\n--- Triggering Compaction ---");
    let compact_resp = client
        .compact(CompactionRequest {
            namespace: None,
            force: false,
        })
        .await?;
    println!(
        "Compaction job: {} — {}",
        compact_resp.job_id, compact_resp.message
    );
    assert!(
        !compact_resp.job_id.is_empty(),
        "expected non-empty compaction job ID"
    );

    // Poll job status
    if let Some(job) = client.get_job(&compact_resp.job_id).await? {
        println!(
            "Job {} status: {} ({}%)",
            job.id, job.status, job.progress
        );
    }

    // =========================================================================
    // Cache Statistics
    // =========================================================================

    println!("\n--- Cache Stats ---");
    let cache = client.cache_stats().await?;
    println!(
        "Cache: {} entries, {} bytes, hit rate: {:.1}%",
        cache.entries, cache.size_bytes, cache.hit_rate * 100.0
    );
    println!(
        "  Hits: {}, Misses: {}, Evictions: {}",
        cache.hits, cache.misses, cache.evictions
    );

    // =========================================================================
    // Ops Stats (Read-scoped, works with read-only keys)
    // =========================================================================

    println!("\n--- Ops Stats ---");
    let stats = client.ops_stats().await?;
    println!(
        "Version: {}, Total vectors: {}, Namespaces: {}, Uptime: {}s",
        stats.version, stats.total_vectors, stats.namespace_count, stats.uptime_seconds
    );
    assert!(!stats.version.is_empty(), "expected non-empty version");

    // =========================================================================
    // Slow Query Summary
    // =========================================================================

    println!("\n--- Slow Queries ---");
    let slow = client.slow_queries(Some(5), None, None).await?;
    println!(
        "Slow queries (threshold: {:.0}ms): {} total",
        slow.threshold_ms, slow.total
    );
    for sq in &slow.queries {
        println!(
            "  {} — {:.1}ms ({} in {})",
            sq.id, sq.duration_ms, sq.query_type, sq.namespace
        );
    }

    println!("\nAll ops diagnostics completed successfully.");
    Ok(())
}
