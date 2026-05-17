//! Unit tests for analytics and knowledge graph methods using mockito.
//!
//! Covers: analytics_overview, analytics_latency, analytics_throughput,
//! analytics_storage, knowledge_graph, full_knowledge_graph, summarize,
//! deduplicate, cross_agent_network, knowledge_query, knowledge_path,
//! knowledge_export.

use dakera_client::DakeraClient;

// ============================================================================
// Analytics: overview
// ============================================================================

#[tokio::test]
async fn test_analytics_overview() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/analytics/overview")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "total_queries": 150000,
                "avg_latency_ms": 3.2,
                "p95_latency_ms": 8.5,
                "p99_latency_ms": 15.0,
                "queries_per_second": 42.0,
                "error_rate": 0.001,
                "cache_hit_rate": 0.85,
                "storage_used_bytes": 1073741824,
                "total_vectors": 500000,
                "total_namespaces": 12,
                "uptime_seconds": 604800
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let overview = client.analytics_overview(None, None).await.unwrap();
    assert_eq!(overview.total_queries, 150000);
    assert!((overview.avg_latency_ms - 3.2).abs() < 1e-5);
    assert_eq!(overview.total_vectors, 500000);
    assert_eq!(overview.total_namespaces, 12);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_analytics_overview_with_params() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/analytics/overview?period=1h&namespace=test-ns")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "total_queries": 5000,
                "avg_latency_ms": 2.1,
                "p95_latency_ms": 6.0,
                "p99_latency_ms": 12.0,
                "queries_per_second": 1.4,
                "error_rate": 0.0,
                "cache_hit_rate": 0.92,
                "storage_used_bytes": 10485760,
                "total_vectors": 1000,
                "total_namespaces": 1,
                "uptime_seconds": 3600
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let overview = client
        .analytics_overview(Some("1h"), Some("test-ns"))
        .await
        .unwrap();
    assert_eq!(overview.total_queries, 5000);
    assert!((overview.cache_hit_rate - 0.92).abs() < 1e-5);
    mock.assert_async().await;
}

// ============================================================================
// Analytics: latency
// ============================================================================

#[tokio::test]
async fn test_analytics_latency() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/analytics/latency")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "period": "24h",
                "avg_ms": 4.5,
                "p50_ms": 3.0,
                "p95_ms": 10.0,
                "p99_ms": 20.0,
                "max_ms": 150.0,
                "by_operation": {
                    "query": {"avg_ms": 3.5, "p95_ms": 8.0, "count": 10000},
                    "upsert": {"avg_ms": 5.0, "p95_ms": 12.0, "count": 5000}
                }
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let latency = client.analytics_latency(None, None).await.unwrap();
    assert_eq!(latency.period, "24h");
    assert!((latency.avg_ms - 4.5).abs() < 1e-5);
    assert!((latency.p50_ms - 3.0).abs() < 1e-5);
    assert!(latency.by_operation.contains_key("query"));
    assert_eq!(latency.by_operation["query"].count, 10000);
    mock.assert_async().await;
}

// ============================================================================
// Analytics: throughput
// ============================================================================

#[tokio::test]
async fn test_analytics_throughput() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/analytics/throughput")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "period": "1h",
                "total_operations": 25000,
                "operations_per_second": 6.94,
                "by_operation": {
                    "query": 15000,
                    "upsert": 8000,
                    "delete": 2000
                }
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let throughput = client.analytics_throughput(None, None).await.unwrap();
    assert_eq!(throughput.period, "1h");
    assert_eq!(throughput.total_operations, 25000);
    assert_eq!(throughput.by_operation["query"], 15000);
    mock.assert_async().await;
}

// ============================================================================
// Analytics: storage
// ============================================================================

#[tokio::test]
async fn test_analytics_storage() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/analytics/storage")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "total_bytes": 10737418240,
                "index_bytes": 2147483648,
                "data_bytes": 8589934592,
                "by_namespace": {
                    "ns1": {"bytes": 5368709120, "vector_count": 250000},
                    "ns2": {"bytes": 5368709120, "vector_count": 250000}
                }
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let storage = client.analytics_storage(None).await.unwrap();
    assert_eq!(storage.total_bytes, 10737418240);
    assert_eq!(storage.index_bytes, 2147483648);
    assert!(storage.by_namespace.contains_key("ns1"));
    assert_eq!(storage.by_namespace["ns1"].vector_count, 250000);
    mock.assert_async().await;
}

// ============================================================================
// Knowledge Graph: knowledge_graph
// ============================================================================

#[tokio::test]
async fn test_knowledge_graph() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/knowledge/graph")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "nodes": [
                    {"id": "mem-1", "content": "User likes coffee", "importance": 0.8, "metadata": {}},
                    {"id": "mem-2", "content": "User prefers dark roast", "importance": 0.7, "metadata": {}}
                ],
                "edges": [
                    {"source": "mem-1", "target": "mem-2", "similarity": 0.85}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::KnowledgeGraphRequest {
        agent_id: "agent-1".to_string(),
        memory_id: Some("mem-1".to_string()),
        depth: Some(2),
        min_similarity: Some(0.5),
    };
    let graph = client.knowledge_graph(request).await.unwrap();
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].source, "mem-1");
    assert!((graph.edges[0].similarity - 0.85).abs() < 1e-5);
    mock.assert_async().await;
}

// ============================================================================
// Knowledge Graph: full_knowledge_graph
// ============================================================================

#[tokio::test]
async fn test_full_knowledge_graph() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/knowledge/graph/full")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "nodes": [
                    {"id": "n1", "content": "Node 1", "importance": 0.9, "metadata": {}},
                    {"id": "n2", "content": "Node 2", "importance": 0.6, "metadata": {}},
                    {"id": "n3", "content": "Node 3", "importance": 0.7, "metadata": {}}
                ],
                "edges": [
                    {"source": "n1", "target": "n2", "similarity": 0.75},
                    {"source": "n2", "target": "n3", "similarity": 0.62}
                ],
                "clusters": [["n1", "n2"], ["n3"]]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::FullKnowledgeGraphRequest {
        agent_id: "agent-1".to_string(),
        max_nodes: Some(100),
        min_similarity: Some(0.3),
        cluster_threshold: Some(0.6),
        max_edges_per_node: Some(5),
    };
    let graph = client.full_knowledge_graph(request).await.unwrap();
    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.edges.len(), 2);
    assert!(graph.clusters.is_some());
    assert_eq!(graph.clusters.unwrap().len(), 2);
    mock.assert_async().await;
}

// ============================================================================
// Knowledge Graph: summarize
// ============================================================================

#[tokio::test]
async fn test_summarize() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/knowledge/summarize")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "summary": "User preferences include dark mode, coffee, and programming.",
                "source_count": 5,
                "new_memory_id": "mem-summary-1"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::SummarizeRequest {
        agent_id: "agent-1".to_string(),
        memory_ids: Some(vec!["m1".to_string(), "m2".to_string()]),
        target_type: Some("semantic".to_string()),
        dry_run: false,
    };
    let result = client.summarize(request).await.unwrap();
    assert_eq!(result.source_count, 5);
    assert!(result.summary.contains("preferences"));
    assert_eq!(result.new_memory_id, Some("mem-summary-1".to_string()));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_summarize_dry_run() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/knowledge/summarize")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "summary": "Dry run summary output.",
                "source_count": 3
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::SummarizeRequest {
        agent_id: "agent-1".to_string(),
        memory_ids: None,
        target_type: None,
        dry_run: true,
    };
    let result = client.summarize(request).await.unwrap();
    assert_eq!(result.source_count, 3);
    assert!(result.new_memory_id.is_none());
    mock.assert_async().await;
}

// ============================================================================
// Knowledge Graph: deduplicate
// ============================================================================

#[tokio::test]
async fn test_deduplicate() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/knowledge/deduplicate")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "duplicates_found": 4,
                "removed_count": 3,
                "groups": [["m1", "m2"], ["m3", "m4"]]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::DeduplicateRequest {
        agent_id: "agent-1".to_string(),
        threshold: Some(0.92),
        memory_type: Some("episodic".to_string()),
        dry_run: false,
    };
    let result = client.deduplicate(request).await.unwrap();
    assert_eq!(result.duplicates_found, 4);
    assert_eq!(result.removed_count, 3);
    assert_eq!(result.groups.len(), 2);
    mock.assert_async().await;
}

// ============================================================================
// Knowledge Graph: cross_agent_network
// ============================================================================

#[tokio::test]
async fn test_cross_agent_network() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/knowledge/network/cross-agent")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "agents": [
                    {"agent_id": "a1", "memory_count": 100, "avg_importance": 0.7},
                    {"agent_id": "a2", "memory_count": 80, "avg_importance": 0.6}
                ],
                "nodes": [
                    {"id": "n1", "agent_id": "a1", "content": "shared concept", "importance": 0.8, "tags": ["concept"], "memory_type": "semantic", "created_at": 1700000000}
                ],
                "edges": [
                    {"source": "n1", "target": "n2", "source_agent": "a1", "target_agent": "a2", "similarity": 0.72}
                ],
                "stats": {
                    "total_agents": 2,
                    "total_nodes": 180,
                    "total_cross_edges": 15,
                    "density": 0.05
                },
                "node_count": 180
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::CrossAgentNetworkRequest {
        agent_ids: Some(vec!["a1".to_string(), "a2".to_string()]),
        min_similarity: 0.5,
        max_nodes_per_agent: 50,
        min_importance: 0.3,
        max_cross_edges: 100,
    };
    let result = client.cross_agent_network(request).await.unwrap();
    assert_eq!(result.agents.len(), 2);
    assert_eq!(result.stats.total_agents, 2);
    assert_eq!(result.stats.total_cross_edges, 15);
    assert_eq!(result.node_count, 180);
    mock.assert_async().await;
}

// ============================================================================
// Knowledge Graph: knowledge_query (KG-2)
// ============================================================================

#[tokio::test]
async fn test_knowledge_query() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock(
            "GET",
            "/v1/knowledge/query?agent_id=agent-1&root_id=mem-1&min_weight=0.5&max_depth=2&limit=50",
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "agent_id": "agent-1",
                "node_count": 2,
                "edge_count": 1,
                "edges": [{"id": "e1", "source_id": "mem-1", "target_id": "mem-2", "edge_type": "related_to", "weight": 0.8, "created_at": 1700000000}]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client
        .knowledge_query("agent-1", Some("mem-1"), None, Some(0.5), Some(2), Some(50))
        .await
        .unwrap();
    assert_eq!(result.edge_count, 1);
    assert_eq!(result.node_count, 2);
    mock.assert_async().await;
}

// ============================================================================
// Knowledge Graph: knowledge_path (KG-2)
// ============================================================================

#[tokio::test]
async fn test_knowledge_path() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock(
            "GET",
            "/v1/knowledge/path?agent_id=agent-1&from=mem-1&to=mem-5",
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "agent_id": "agent-1",
                "from_id": "mem-1",
                "to_id": "mem-5",
                "hop_count": 2,
                "path": ["mem-1", "mem-3", "mem-5"]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client
        .knowledge_path("agent-1", "mem-1", "mem-5")
        .await
        .unwrap();
    assert_eq!(result.path.len(), 3);
    assert_eq!(result.hop_count, 2);
    mock.assert_async().await;
}

// ============================================================================
// Knowledge Graph: knowledge_export (KG-2)
// ============================================================================

#[tokio::test]
async fn test_knowledge_export() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/knowledge/export?agent_id=agent-1&format=json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "agent_id": "agent-1",
                "format": "json",
                "node_count": 1,
                "edge_count": 0,
                "edges": []
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client
        .knowledge_export("agent-1", Some("json"))
        .await
        .unwrap();
    assert_eq!(result.format, "json");
    assert_eq!(result.node_count, 1);
    assert_eq!(result.edge_count, 0);
    mock.assert_async().await;
}
