//! Knowledge graph — KG build, traverse, query, path, export
//!
//! Run: cargo run --example knowledge_graph

use dakera_client::{
    DakeraClient, FullKnowledgeGraphRequest, KnowledgeGraphRequest, MemoryType, StoreMemoryRequest,
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

    let agent_id = "kg-example-agent";

    // =========================================================================
    // Store Memories to Build Graph From
    // =========================================================================

    println!("\n--- Storing memories for knowledge graph ---");
    let memories = vec![
        StoreMemoryRequest::new(agent_id, "Rust provides memory safety without garbage collection")
            .with_type(MemoryType::Semantic)
            .with_importance(0.9)
            .with_tags(vec!["rust".into(), "memory-safety".into()]),
        StoreMemoryRequest::new(agent_id, "HNSW is an efficient algorithm for approximate nearest neighbor search")
            .with_type(MemoryType::Semantic)
            .with_importance(0.8)
            .with_tags(vec!["algorithms".into(), "search".into()]),
        StoreMemoryRequest::new(agent_id, "Vector databases use embeddings to represent semantic similarity")
            .with_type(MemoryType::Semantic)
            .with_importance(0.85)
            .with_tags(vec!["vectors".into(), "embeddings".into()]),
        StoreMemoryRequest::new(agent_id, "Knowledge graphs connect related concepts through typed edges")
            .with_type(MemoryType::Semantic)
            .with_importance(0.9)
            .with_tags(vec!["knowledge-graph".into(), "relationships".into()]),
        StoreMemoryRequest::new(agent_id, "BM25 scoring ranks documents by term frequency and inverse document frequency")
            .with_type(MemoryType::Semantic)
            .with_importance(0.7)
            .with_tags(vec!["search".into(), "ranking".into()]),
    ];

    let mut memory_ids = Vec::new();
    for mem in memories {
        let resp = client.store_memory(mem).await?;
        println!("  Stored: {}", resp.memory_id);
        memory_ids.push(resp.memory_id);
    }
    assert_eq!(memory_ids.len(), 5, "expected 5 memories stored");

    // =========================================================================
    // Build Knowledge Graph (from seed memory)
    // =========================================================================

    println!("\n--- Knowledge Graph (from seed) ---");
    let kg = client
        .knowledge_graph(KnowledgeGraphRequest {
            agent_id: agent_id.to_string(),
            memory_id: Some(memory_ids[0].clone()),
            depth: Some(2),
            min_similarity: Some(0.1),
        })
        .await?;
    println!(
        "Graph: {} nodes, {} edges",
        kg.nodes.len(),
        kg.edges.len()
    );
    for node in &kg.nodes {
        println!("  Node: {} — {:.50}...", node.id, node.content);
    }
    assert!(!kg.nodes.is_empty(), "expected non-empty knowledge graph nodes");

    // =========================================================================
    // Full Knowledge Graph
    // =========================================================================

    println!("\n--- Full Knowledge Graph ---");
    let full_kg = client
        .full_knowledge_graph(FullKnowledgeGraphRequest {
            agent_id: agent_id.to_string(),
            max_nodes: Some(50),
            min_similarity: Some(0.1),
            cluster_threshold: Some(0.5),
            max_edges_per_node: Some(5),
        })
        .await?;
    println!(
        "Full graph: {} nodes, {} edges, {} clusters",
        full_kg.nodes.len(),
        full_kg.edges.len(),
        full_kg.clusters.as_ref().map_or(0, |c| c.len())
    );

    // =========================================================================
    // KG Query (filter by edge type and weight)
    // =========================================================================

    println!("\n--- KG Query ---");
    let query_resp = client
        .knowledge_query(
            agent_id,
            None,              // no root — return all edges
            None,              // all edge types
            Some(0.1),         // min weight
            None,              // default depth
            Some(20),          // limit
        )
        .await?;
    println!(
        "Query: {} nodes, {} edges",
        query_resp.node_count, query_resp.edge_count
    );

    // =========================================================================
    // KG Path (shortest path between two memories)
    // =========================================================================

    if memory_ids.len() >= 2 {
        println!("\n--- KG Path ---");
        let path_resp = client
            .knowledge_path(agent_id, &memory_ids[0], &memory_ids[3])
            .await?;
        println!(
            "Path from {} to {}: {} hops",
            path_resp.from_id,
            path_resp.to_id,
            path_resp.path.len().saturating_sub(1)
        );
        for node_id in &path_resp.path {
            println!("  -> {}", node_id);
        }
    }

    // =========================================================================
    // KG Export
    // =========================================================================

    println!("\n--- KG Export (JSON) ---");
    let export = client.knowledge_export(agent_id, Some("json")).await?;
    println!(
        "Exported graph: {} nodes, {} edges (format: {})",
        export.node_count, export.edge_count, export.format
    );
    assert!(export.node_count > 0, "expected non-empty export");

    println!("\nKnowledge graph example completed successfully.");
    Ok(())
}
