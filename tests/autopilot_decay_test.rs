//! Integration tests for AutoPilot (PILOT-1/2/3) and Decay (DECAY-1/2/3) APIs.
//!
//! These tests cover methods added in v0.7.2 (AutoPilot) and v0.7.3 (Decay),
//! which shipped without test coverage. Uses mockito to mock HTTP responses.

use dakera_client::{
    admin::{AutoPilotConfigRequest, AutoPilotTriggerAction, DecayConfigUpdateRequest},
    memory::StoreMemoryRequest,
    DakeraClient,
};

// ============================================================================
// AutoPilot Status (PILOT-1)
// ============================================================================

#[tokio::test]
async fn test_autopilot_status_gets_correct_endpoint() {
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
                "last_dedup_at": 1700000000,
                "total_dedup_removed": 42,
                "total_consolidated": 10
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.autopilot_status().await.unwrap();

    assert!(result.config.enabled);
    assert_eq!(result.config.dedup_threshold, 0.93);
    assert_eq!(result.total_dedup_removed, 42);
    assert_eq!(result.total_consolidated, 10);
    mock.assert_async().await;
}

// ============================================================================
// AutoPilot Update Config (PILOT-2)
// ============================================================================

#[tokio::test]
async fn test_autopilot_update_config_puts_correct_endpoint() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("PUT", "/admin/autopilot/config")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "success": true,
                "config": {
                    "enabled": false,
                    "dedup_threshold": 0.90,
                    "dedup_interval_hours": 8,
                    "consolidation_interval_hours": 24
                },
                "message": "AutoPilot config updated"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = AutoPilotConfigRequest {
        enabled: Some(false),
        dedup_threshold: Some(0.90),
        ..Default::default()
    };
    let result = client.autopilot_update_config(req).await.unwrap();

    assert!(result.success);
    assert!(!result.config.enabled);
    assert_eq!(result.config.dedup_threshold, 0.90);
    assert_eq!(result.message, "AutoPilot config updated");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_autopilot_update_config_omits_unset_fields() {
    let mut server = mockito::Server::new_async().await;
    // Verify the request body only contains the field we set
    let mock = server
        .mock("PUT", "/admin/autopilot/config")
        .match_body(mockito::Matcher::PartialJsonString(
            r#"{"dedup_interval_hours":4}"#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"success": true, "config": {"enabled": true, "dedup_threshold": 0.93, "dedup_interval_hours": 4, "consolidation_interval_hours": 12}, "message": "ok"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = AutoPilotConfigRequest {
        dedup_interval_hours: Some(4),
        ..Default::default()
    };
    client.autopilot_update_config(req).await.unwrap();

    mock.assert_async().await;
}

// ============================================================================
// AutoPilot Trigger (PILOT-3)
// ============================================================================

#[tokio::test]
async fn test_autopilot_trigger_dedup() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/autopilot/trigger")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "success": true,
                "action": "dedup",
                "dedup": {
                    "namespaces_processed": 3,
                    "memories_scanned": 500,
                    "duplicates_removed": 12
                },
                "message": "Dedup cycle completed"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client
        .autopilot_trigger(AutoPilotTriggerAction::Dedup)
        .await
        .unwrap();

    assert!(result.success);
    assert!(matches!(result.action, AutoPilotTriggerAction::Dedup));
    let dedup = result.dedup.unwrap();
    assert_eq!(dedup.duplicates_removed, 12);
    assert_eq!(dedup.namespaces_processed, 3);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_autopilot_trigger_all_returns_both_results() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/admin/autopilot/trigger")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "success": true,
                "action": "all",
                "dedup": {
                    "namespaces_processed": 2,
                    "memories_scanned": 300,
                    "duplicates_removed": 5
                },
                "consolidation": {
                    "namespaces_processed": 2,
                    "memories_scanned": 300,
                    "clusters_merged": 4,
                    "memories_consolidated": 8
                },
                "message": "Full AutoPilot cycle completed"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client
        .autopilot_trigger(AutoPilotTriggerAction::All)
        .await
        .unwrap();

    assert!(matches!(result.action, AutoPilotTriggerAction::All));
    let consolidation = result.consolidation.unwrap();
    assert_eq!(consolidation.clusters_merged, 4);
    assert_eq!(consolidation.memories_consolidated, 8);
    mock.assert_async().await;
}

// ============================================================================
// Decay Config (DECAY-1)
// ============================================================================

#[tokio::test]
async fn test_decay_config_gets_correct_endpoint() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/decay/config")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "strategy": "exponential",
                "half_life_hours": 168.0,
                "min_importance": 0.05
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.decay_config().await.unwrap();

    assert_eq!(result.strategy, "exponential");
    assert_eq!(result.half_life_hours, 168.0);
    assert_eq!(result.min_importance, 0.05);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_decay_update_config_puts_correct_endpoint() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("PUT", "/admin/decay/config")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "success": true,
                "config": {
                    "strategy": "linear",
                    "half_life_hours": 72.0,
                    "min_importance": 0.1
                },
                "message": "Decay config updated"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = DecayConfigUpdateRequest {
        strategy: Some("linear".to_string()),
        half_life_hours: Some(72.0),
        ..Default::default()
    };
    let result = client.decay_update_config(req).await.unwrap();

    assert!(result.success);
    assert_eq!(result.config.strategy, "linear");
    assert_eq!(result.config.half_life_hours, 72.0);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_decay_update_config_omits_unset_fields() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("PUT", "/admin/decay/config")
        .match_body(mockito::Matcher::PartialJsonString(
            r#"{"min_importance":0.02}"#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"success": true, "config": {"strategy": "exponential", "half_life_hours": 168.0, "min_importance": 0.02}, "message": "ok"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = DecayConfigUpdateRequest {
        min_importance: Some(0.02),
        ..Default::default()
    };
    client.decay_update_config(req).await.unwrap();

    mock.assert_async().await;
}

// ============================================================================
// Decay Stats (DECAY-2)
// ============================================================================

#[tokio::test]
async fn test_decay_stats_gets_correct_endpoint() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/decay/stats")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "total_decayed": 1024,
                "total_deleted": 128,
                "last_run_at": 1700000000,
                "cycles_run": 42,
                "last_cycle": {
                    "namespaces_processed": 5,
                    "memories_processed": 200,
                    "memories_decayed": 30,
                    "memories_deleted": 5
                }
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.decay_stats().await.unwrap();

    assert_eq!(result.total_decayed, 1024);
    assert_eq!(result.total_deleted, 128);
    assert_eq!(result.cycles_run, 42);
    let last_cycle = result.last_cycle.unwrap();
    assert_eq!(last_cycle.memories_decayed, 30);
    assert_eq!(last_cycle.namespaces_processed, 5);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_decay_stats_handles_never_run_state() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/admin/decay/stats")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"total_decayed": 0, "total_deleted": 0, "cycles_run": 0}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.decay_stats().await.unwrap();

    assert_eq!(result.cycles_run, 0);
    assert!(result.last_cycle.is_none());
    assert!(result.last_run_at.is_none());
    mock.assert_async().await;
}

// ============================================================================
// store_memory expires_at (DECAY-3)
// ============================================================================

#[tokio::test]
async fn test_store_memory_with_expires_at_includes_field() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/memory/store")
        .match_body(mockito::Matcher::PartialJsonString(
            r#"{"expires_at":1800000000}"#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"memory_id": "mem_1", "agent_id": "agent-1", "namespace": "default"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = StoreMemoryRequest::new("agent-1", "test").with_expires_at(1800000000);
    client.store_memory(req).await.unwrap();

    mock.assert_async().await;
}

#[tokio::test]
async fn test_store_memory_without_expires_at_omits_field() {
    let mut server = mockito::Server::new_async().await;
    // Verify expires_at is NOT in the body
    // Just match on content — expires_at is absent because skip_serializing_if = "Option::is_none"
    let mock = server
        .mock("POST", "/v1/memory/store")
        .match_body(mockito::Matcher::PartialJsonString(
            r#"{"content":"test"}"#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"memory_id": "mem_1", "agent_id": "agent-1", "namespace": "default"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = StoreMemoryRequest::new("agent-1", "test");
    client.store_memory(req).await.unwrap();

    mock.assert_async().await;
}
