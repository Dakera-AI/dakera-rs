//! Unit tests for admin and ops methods using mockito.
//!
//! Covers: diagnostics, list_jobs, get_job, compact, shutdown, cluster_status,
//! cache_stats, cache_clear, list_backups, create_backup, storage_tier_overview,
//! background_activity, memory_type_stats, migrate_namespace_dimensions,
//! download_backup, upload_backup, ops_stats, autopilot_status, decay_config,
//! decay_stats, get_kpis, admin_fulltext_reindex.

use dakera_client::DakeraClient;

// ============================================================================
// Ops: diagnostics
// ============================================================================

#[tokio::test]
async fn test_diagnostics() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/ops/diagnostics")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"system":{"version":"0.11.55","rust_version":"1.78","uptime_seconds":3600,"pid":1234},"resources":{"memory_bytes":1048576,"thread_count":8,"open_fds":64,"cpu_percent":12.5},"components":{"storage":{"healthy":true,"message":"ok","last_check":1700000000},"search_engine":{"healthy":true,"message":"ok","last_check":1700000000},"cache":{"healthy":true,"message":"ok","last_check":1700000000},"grpc":{"healthy":true,"message":"ok","last_check":1700000000}},"active_jobs":2}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.diagnostics().await.unwrap();
    assert_eq!(result.active_jobs, 2);
    assert_eq!(result.resources.memory_bytes, 1048576);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_ops_diagnostics() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/ops/diagnostics")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status":"ok","uptime":3600}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.ops_diagnostics().await.unwrap();
    assert_eq!(result["status"], "ok");
    mock.assert_async().await;
}

// ============================================================================
// Ops: list_jobs
// ============================================================================

#[tokio::test]
async fn test_list_jobs() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/ops/jobs")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"id":"job-1","job_type":"compaction","status":"running","progress":50,"created_at":1700000000},
                {"id":"job-2","job_type":"backup","status":"completed","progress":100,"created_at":1700000100}
            ]"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let jobs = client.list_jobs().await.unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].id, "job-1");
    assert_eq!(jobs[1].status, "completed");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_ops_list_jobs() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/ops/jobs")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"id":"j-100","job_type":"reindex","status":"pending","progress":0,"created_at":1700000200}]"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let jobs = client.ops_list_jobs().await.unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, "j-100");
    mock.assert_async().await;
}

// ============================================================================
// Ops: get_job
// ============================================================================

#[tokio::test]
async fn test_get_job_found() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/ops/jobs/job-1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id":"job-1","job_type":"compaction","status":"running","progress":75,"created_at":1700000000}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let job = client.get_job("job-1").await.unwrap();
    assert!(job.is_some());
    let job = job.unwrap();
    assert_eq!(job.id, "job-1");
    assert_eq!(job.status, "running");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_get_job_not_found() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/ops/jobs/nonexistent")
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"Job not found"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let job = client.get_job("nonexistent").await.unwrap();
    assert!(job.is_none());
    mock.assert_async().await;
}

// ============================================================================
// Ops: compact
// ============================================================================

#[tokio::test]
async fn test_compact() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/ops/compact")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"job_id":"compact-001","message":"Compaction started"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::CompactionRequest {
        namespace: Some("test-ns".to_string()),
        force: true,
    };
    let result = client.compact(request).await.unwrap();
    assert_eq!(result.job_id, "compact-001");
    assert_eq!(result.message, "Compaction started");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_ops_compact() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/ops/compact")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"job_id":"compact-002","message":"Compaction queued"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::CompactionRequest {
        namespace: None,
        force: false,
    };
    let result = client.ops_compact(request).await.unwrap();
    assert_eq!(result.job_id, "compact-002");
    mock.assert_async().await;
}

// ============================================================================
// Ops: shutdown
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/ops/shutdown")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#""#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    client.shutdown().await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn test_ops_shutdown() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/ops/shutdown")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message":"shutting down"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.ops_shutdown().await.unwrap();
    assert_eq!(result["message"], "shutting down");
    mock.assert_async().await;
}

// ============================================================================
// Admin: cluster_status
// ============================================================================

#[tokio::test]
async fn test_cluster_status() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/cluster/status")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "cluster_id": "cluster-1",
                "state": "healthy",
                "node_count": 3,
                "total_vectors": 50000,
                "namespace_count": 5,
                "version": "0.11.54",
                "timestamp": 1700000000,
                "redis_healthy": true
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let status = client.cluster_status().await.unwrap();
    assert_eq!(status.cluster_id, "cluster-1");
    assert_eq!(status.state, "healthy");
    assert_eq!(status.node_count, 3);
    assert_eq!(status.total_vectors, 50000);
    assert_eq!(status.redis_healthy, Some(true));
    mock.assert_async().await;
}

// ============================================================================
// Admin: ops_stats
// ============================================================================

#[tokio::test]
async fn test_ops_stats() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/ops/stats")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "version": "0.11.54",
                "total_vectors": 12345,
                "namespace_count": 8,
                "uptime_seconds": 86400,
                "timestamp": 1700000000,
                "state": "running"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let stats = client.ops_stats().await.unwrap();
    assert_eq!(stats.version, "0.11.54");
    assert_eq!(stats.total_vectors, 12345);
    assert_eq!(stats.namespace_count, 8);
    assert_eq!(stats.uptime_seconds, 86400);
    mock.assert_async().await;
}

// ============================================================================
// Admin: cache_stats and cache_clear
// ============================================================================

#[tokio::test]
async fn test_cache_stats() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/cache/stats")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "enabled": true,
                "cache_type": "lru",
                "entries": 1024,
                "size_bytes": 4194304,
                "hits": 5000,
                "misses": 500,
                "hit_rate": 0.909,
                "evictions": 100
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let stats = client.cache_stats().await.unwrap();
    assert!(stats.enabled);
    assert_eq!(stats.entries, 1024);
    assert_eq!(stats.hits, 5000);
    assert!((stats.hit_rate - 0.909).abs() < 1e-5);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_cache_clear() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/cache/clear")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"success":true,"entries_cleared":512,"message":"Cache cleared"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.cache_clear(Some("test-ns")).await.unwrap();
    assert!(result.success);
    assert_eq!(result.entries_cleared, 512);
    mock.assert_async().await;
}

// ============================================================================
// Admin: backups
// ============================================================================

#[tokio::test]
async fn test_list_backups() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/backups")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "backups": [{
                    "backup_id": "bk-1",
                    "name": "daily-backup",
                    "backup_type": "full",
                    "status": "completed",
                    "namespaces": ["ns1", "ns2"],
                    "vector_count": 10000,
                    "size_bytes": 50000000,
                    "created_at": 1700000000,
                    "encrypted": true
                }],
                "total": 1
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.list_backups().await.unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.backups[0].backup_id, "bk-1");
    assert!(result.backups[0].encrypted);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_create_backup() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/backups")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "backup": {
                    "backup_id": "bk-new",
                    "name": "manual-backup",
                    "backup_type": "full",
                    "status": "in_progress",
                    "namespaces": ["ns1"],
                    "vector_count": 0,
                    "size_bytes": 0,
                    "created_at": 1700001000,
                    "encrypted": false
                },
                "estimated_completion": 1700001060
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::CreateBackupRequest {
        name: "manual-backup".to_string(),
        backup_type: Some("full".to_string()),
        namespaces: Some(vec!["ns1".to_string()]),
        encrypt: None,
        compression: None,
    };
    let result = client.create_backup(request).await.unwrap();
    assert_eq!(result.backup.backup_id, "bk-new");
    assert_eq!(result.estimated_completion, Some(1700001060));
    mock.assert_async().await;
}

// ============================================================================
// Admin: download_backup / upload_backup
// ============================================================================

#[tokio::test]
async fn test_download_backup() {
    let mut server = mockito::Server::new_async().await;
    let data = vec![0x1f, 0x8b, 0x08, 0x00]; // fake gzip header
    let mock = server
        .mock("GET", "/admin/backups/bk-42/download")
        .with_status(200)
        .with_header("content-type", "application/gzip")
        .with_body(data.clone())
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.download_backup("bk-42").await.unwrap();
    assert_eq!(result, data);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_upload_backup() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/backups/upload")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "backup": {
                    "backup_id": "bk-uploaded",
                    "name": "uploaded",
                    "backup_type": "full",
                    "status": "completed",
                    "namespaces": [],
                    "vector_count": 500,
                    "size_bytes": 1024,
                    "created_at": 1700002000,
                    "encrypted": false
                }
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let data = vec![0x1f, 0x8b, 0x08, 0x00, 0x00];
    let result = client.upload_backup(data).await.unwrap();
    assert_eq!(result.backup.backup_id, "bk-uploaded");
    mock.assert_async().await;
}

// ============================================================================
// Admin: storage_tier_overview
// ============================================================================

#[tokio::test]
async fn test_storage_tier_overview() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/storage/tiers")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "tiers_enabled": true,
                "architecture": [],
                "config": {"hot_tier_capacity": 10000, "hot_to_warm_threshold_secs": 3600, "warm_to_cold_threshold_secs": 86400, "auto_tier_enabled": true, "tier_check_interval_secs": 300},
                "activity": {"promotions": 10, "demotions": 5, "cache_hit_rate": 0.85, "storage_backend": "mmap", "promotions_to_hot": 8, "demotions_to_warm": 3, "demotions_to_cold": 2}
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.storage_tier_overview().await.unwrap();
    assert!(result.tiers_enabled);
    mock.assert_async().await;
}

// ============================================================================
// Admin: background_activity
// ============================================================================

#[tokio::test]
async fn test_background_activity() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/background-activity")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"active_tasks":3,"completed_tasks":42,"failed_tasks":1}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.background_activity().await.unwrap();
    assert_eq!(result["active_tasks"], 3);
    assert_eq!(result["completed_tasks"], 42);
    mock.assert_async().await;
}

// ============================================================================
// Admin: memory_type_stats
// ============================================================================

#[tokio::test]
async fn test_memory_type_stats() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/memory-type-stats")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "total": 5000,
                "working": 200,
                "episodic": 3000,
                "semantic": 1500,
                "procedural": 300,
                "agent_namespaces": 12
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.memory_type_stats().await.unwrap();
    assert_eq!(result.total, 5000);
    assert_eq!(result.episodic, 3000);
    assert_eq!(result.semantic, 1500);
    assert_eq!(result.agent_namespaces, 12);
    mock.assert_async().await;
}

// ============================================================================
// Admin: migrate_namespace_dimensions
// ============================================================================

#[tokio::test]
async fn test_migrate_namespace_dimensions() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/namespaces/migrate-dimensions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "migrated": 2,
                "failed": 0,
                "already_current": 1,
                "results": [
                    {"namespace":"ns1","original_dimension":384,"vectors_migrated":100,"vectors_skipped":0,"status":"completed"},
                    {"namespace":"ns2","original_dimension":384,"vectors_migrated":50,"vectors_skipped":5,"status":"completed"}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::MigrateNamespaceDimensionsRequest {
        namespaces: vec!["ns1".to_string(), "ns2".to_string()],
        target_dimension: 1024,
    };
    let result = client.migrate_namespace_dimensions(request).await.unwrap();
    assert_eq!(result.migrated, 2);
    assert_eq!(result.failed, 0);
    assert_eq!(result.results.len(), 2);
    mock.assert_async().await;
}

// ============================================================================
// Admin: autopilot_status
// ============================================================================

#[tokio::test]
async fn test_autopilot_status() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/autopilot/status")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "config": {
                    "enabled": true,
                    "dedup_threshold": 0.93,
                    "dedup_interval_hours": 6,
                    "consolidation_interval_hours": 12
                },
                "total_dedup_removed": 150,
                "total_consolidated": 30
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let status = client.autopilot_status().await.unwrap();
    assert!(status.config.enabled);
    assert_eq!(status.total_dedup_removed, 150);
    assert_eq!(status.total_consolidated, 30);
    mock.assert_async().await;
}

// ============================================================================
// Admin: decay_config and decay_stats
// ============================================================================

#[tokio::test]
async fn test_decay_config() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/decay/config")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"strategy":"exponential","half_life_hours":168.0,"min_importance":0.05}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let config = client.decay_config().await.unwrap();
    assert_eq!(config.strategy, "exponential");
    assert!((config.half_life_hours - 168.0).abs() < 1e-5);
    assert!((config.min_importance - 0.05).abs() < 1e-5);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_decay_stats() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/decay/stats")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "total_decayed": 500,
                "total_deleted": 50,
                "last_run_at": 1700000000,
                "cycles_run": 24
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let stats = client.decay_stats().await.unwrap();
    assert_eq!(stats.total_decayed, 500);
    assert_eq!(stats.total_deleted, 50);
    assert_eq!(stats.cycles_run, 24);
    assert_eq!(stats.last_run_at, Some(1700000000));
    mock.assert_async().await;
}

// ============================================================================
// Admin: get_kpis
// ============================================================================

#[tokio::test]
async fn test_get_kpis() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/kpis")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "recall_latency_p50_ms": 2.5,
                "recall_latency_p99_ms": 15.0,
                "store_latency_p50_ms": 5.0,
                "api_error_rate_5xx_pct": 0.1,
                "active_agents_count": 42,
                "session_count_week": 1500,
                "cross_agent_network_node_count": 300,
                "memory_retention_7d_pct": 95.2
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let kpis = client.get_kpis().await.unwrap();
    assert!((kpis.recall_latency_p50_ms - 2.5).abs() < 1e-5);
    assert_eq!(kpis.active_agents_count, 42);
    assert_eq!(kpis.session_count_week, 1500);
    mock.assert_async().await;
}

// ============================================================================
// Admin: admin_fulltext_reindex
// ============================================================================

#[tokio::test]
async fn test_admin_fulltext_reindex() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/fulltext/reindex")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "namespaces_processed": 3,
                "total_indexed": 150,
                "total_skipped": 800,
                "details": [
                    {"namespace":"agent-1","vectors_scanned":400,"newly_indexed":50,"already_indexed":350,"parse_failures":0},
                    {"namespace":"agent-2","vectors_scanned":300,"newly_indexed":100,"already_indexed":200,"parse_failures":0},
                    {"namespace":"agent-3","vectors_scanned":250,"newly_indexed":0,"already_indexed":250,"parse_failures":0}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.admin_fulltext_reindex(None).await.unwrap();
    assert_eq!(result.namespaces_processed, 3);
    assert_eq!(result.total_indexed, 150);
    assert_eq!(result.total_skipped, 800);
    assert_eq!(result.details.len(), 3);
    mock.assert_async().await;
}

// ============================================================================
// Error: 403 on admin endpoint
// ============================================================================

#[tokio::test]
async fn test_cluster_status_authorization_error() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/cluster/status")
        .with_status(403)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"Admin scope required","code":"AUTHORIZATION_ERROR"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let err = client.cluster_status().await.unwrap_err();
    assert!(
        matches!(err, dakera_client::ClientError::Authorization { .. }),
        "expected Authorization error, got: {err:?}"
    );
    mock.assert_async().await;
}

// ============================================================================
// Admin: drain_reembed — POST /admin/reembed/drain (v0.11.82+, DAK-6326)
// ============================================================================

#[tokio::test]
async fn test_drain_reembed_full_drain() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/reembed/drain")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"processed":1280,"remaining":0,"elapsed_ms":4210,"cycles":3,"timed_out":false}"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client
        .drain_reembed(dakera_client::DrainReembedRequest::default())
        .await
        .unwrap();
    assert_eq!(result.processed, 1280);
    assert_eq!(result.remaining, 0);
    assert_eq!(result.cycles, 3);
    assert!(!result.timed_out);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_drain_reembed_forwards_params() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/reembed/drain")
        .match_body(mockito::Matcher::PartialJsonString(
            r#"{"timeout_secs":600,"batch_size":5000}"#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"processed":500,"remaining":120,"elapsed_ms":600000,"cycles":50,"timed_out":true}"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::DrainReembedRequest {
        timeout_secs: Some(600),
        batch_size: Some(5000),
        min_importance: Some(0.5),
    };
    let result = client.drain_reembed(request).await.unwrap();
    assert!(result.timed_out);
    assert_eq!(result.remaining, 120);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_drain_reembed_requires_admin_scope() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/reembed/drain")
        .with_status(403)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"Admin scope required","code":"AUTHORIZATION_ERROR"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let err = client
        .drain_reembed(dakera_client::DrainReembedRequest::default())
        .await
        .unwrap_err();
    assert!(
        matches!(err, dakera_client::ClientError::Authorization { .. }),
        "expected Authorization error, got: {err:?}"
    );
    mock.assert_async().await;
}
