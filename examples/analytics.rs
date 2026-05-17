//! Analytics — agent stats, KPIs, analytics, sessions
//!
//! Run: cargo run --example analytics

use dakera_client::{DakeraClient, MemoryType, StoreMemoryRequest};

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

    let agent_id = "analytics-example-agent";

    // Store a few memories so we have data to analyze
    println!("\n--- Seeding memories ---");
    for i in 0..3 {
        let mem = StoreMemoryRequest::new(agent_id, format!("Analytics test memory #{}", i))
            .with_type(MemoryType::Episodic)
            .with_importance(0.5 + i as f32 * 0.15)
            .with_tags(vec!["analytics-test".into()]);
        client.store_memory(mem).await?;
    }
    println!("Stored 3 test memories");

    // =========================================================================
    // Agent Stats
    // =========================================================================

    println!("\n--- Agent Stats ---");
    let stats = client.agent_stats(agent_id).await?;
    println!(
        "Agent '{}': {} memories, {} sessions ({} active)",
        stats.agent_id, stats.total_memories, stats.total_sessions, stats.active_sessions
    );
    if let Some(avg) = stats.avg_importance {
        println!("  Avg importance: {:.2}", avg);
    }
    if !stats.memories_by_type.is_empty() {
        println!("  By type:");
        for (mtype, count) in &stats.memories_by_type {
            println!("    {}: {}", mtype, count);
        }
    }
    assert!(
        stats.total_memories >= 3,
        "expected at least 3 memories for agent"
    );

    // =========================================================================
    // Agent Sessions
    // =========================================================================

    println!("\n--- Agent Sessions ---");
    let sessions = client.agent_sessions(agent_id, None, Some(10)).await?;
    println!("Sessions for '{}': {}", agent_id, sessions.len());
    for session in &sessions {
        let status = if session.ended_at.is_some() {
            "ended"
        } else {
            "active"
        };
        println!(
            "  {} — started: {}, status: {}",
            session.id, session.started_at, status
        );
    }

    // =========================================================================
    // Product KPIs (Admin scope)
    // =========================================================================

    println!("\n--- Product KPIs ---");
    let kpis = client.get_kpis().await?;
    println!(
        "  Recall latency p50: {:.2}ms, p99: {:.2}ms",
        kpis.recall_latency_p50_ms, kpis.recall_latency_p99_ms
    );
    println!("  Store latency p50: {:.2}ms", kpis.store_latency_p50_ms);
    println!(
        "  API error rate (5xx): {:.3}%",
        kpis.api_error_rate_5xx_pct
    );
    println!("  Active agents (24h): {}", kpis.active_agents_count);
    println!("  Sessions (7d): {}", kpis.session_count_week);
    println!(
        "  KG network nodes: {}",
        kpis.cross_agent_network_node_count
    );
    println!(
        "  Memory retention (7d): {:.1}%",
        kpis.memory_retention_7d_pct
    );

    // =========================================================================
    // Analytics Overview
    // =========================================================================

    println!("\n--- Analytics Overview (1h) ---");
    let overview = client.analytics_overview(Some("1h"), None).await?;
    println!(
        "  Queries: {}, QPS: {:.1}, Error rate: {:.3}%",
        overview.total_queries,
        overview.queries_per_second,
        overview.error_rate * 100.0
    );
    println!(
        "  Latency — avg: {:.1}ms, p95: {:.1}ms, p99: {:.1}ms",
        overview.avg_latency_ms, overview.p95_latency_ms, overview.p99_latency_ms
    );
    println!(
        "  Cache hit rate: {:.1}%, Storage: {} bytes",
        overview.cache_hit_rate * 100.0,
        overview.storage_used_bytes
    );

    // =========================================================================
    // Latency Analytics
    // =========================================================================

    println!("\n--- Latency Analytics (1h) ---");
    let latency = client.analytics_latency(Some("1h"), None).await?;
    println!(
        "  Period: {}, p50: {:.1}ms, p95: {:.1}ms, p99: {:.1}ms, max: {:.1}ms",
        latency.period, latency.p50_ms, latency.p95_ms, latency.p99_ms, latency.max_ms
    );
    if !latency.by_operation.is_empty() {
        println!("  By operation:");
        for (op, stats) in &latency.by_operation {
            println!(
                "    {}: avg {:.1}ms, p95 {:.1}ms ({} ops)",
                op, stats.avg_ms, stats.p95_ms, stats.count
            );
        }
    }

    // =========================================================================
    // Throughput Analytics
    // =========================================================================

    println!("\n--- Throughput Analytics (1h) ---");
    let throughput = client.analytics_throughput(Some("1h"), None).await?;
    println!(
        "  Total ops: {}, OPS: {:.1}",
        throughput.total_operations, throughput.operations_per_second
    );
    if !throughput.by_operation.is_empty() {
        for (op, count) in &throughput.by_operation {
            println!("    {}: {}", op, count);
        }
    }

    // =========================================================================
    // Storage Analytics
    // =========================================================================

    println!("\n--- Storage Analytics ---");
    let storage = client.analytics_storage(None).await?;
    println!(
        "  Total: {} bytes (index: {}, data: {})",
        storage.total_bytes, storage.index_bytes, storage.data_bytes
    );
    if !storage.by_namespace.is_empty() {
        println!("  By namespace:");
        for (ns, info) in &storage.by_namespace {
            println!(
                "    {}: {} bytes, {} vectors",
                ns, info.bytes, info.vector_count
            );
        }
    }

    println!("\nAnalytics example completed successfully.");
    Ok(())
}
