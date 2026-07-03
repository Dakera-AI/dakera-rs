//! Tests for RecalledMemory score field priority (smart_score > weighted_score > score).

use dakera_client::RecalledMemory;

fn deserialize(json: &str) -> RecalledMemory {
    serde_json::from_str(json).expect("deserialization failed")
}

#[test]
fn test_smart_score_takes_priority() {
    let json = r#"{
        "memory": {"id": "m1", "content": "test", "memory_type": "episodic", "importance": 0.8, "created_at": 1000, "last_accessed_at": 1000, "access_count": 0, "tags": []},
        "score": 0.5,
        "weighted_score": 0.7,
        "smart_score": 0.9
    }"#;
    let m = deserialize(json);
    assert_eq!(m.score, 0.9, ".score must equal smart_score when present");
    assert_eq!(m.smart_score, Some(0.9));
    assert_eq!(m.weighted_score, Some(0.7));
}

#[test]
fn test_weighted_score_used_when_no_smart_score() {
    let json = r#"{
        "memory": {"id": "m2", "content": "test", "memory_type": "episodic", "importance": 0.8, "created_at": 1000, "last_accessed_at": 1000, "access_count": 0, "tags": []},
        "score": 0.5,
        "weighted_score": 0.7
    }"#;
    let m = deserialize(json);
    assert_eq!(m.score, 0.7, ".score must equal weighted_score when smart_score absent");
    assert_eq!(m.smart_score, None);
    assert_eq!(m.weighted_score, Some(0.7));
}

#[test]
fn test_raw_score_fallback() {
    let json = r#"{
        "memory": {"id": "m3", "content": "test", "memory_type": "episodic", "importance": 0.8, "created_at": 1000, "last_accessed_at": 1000, "access_count": 0, "tags": []},
        "score": 0.5
    }"#;
    let m = deserialize(json);
    assert_eq!(m.score, 0.5, ".score must equal raw score when no smart_score or weighted_score");
    assert_eq!(m.smart_score, None);
    assert_eq!(m.weighted_score, None);
}

#[test]
fn test_flat_format_score_fallback() {
    // Flat format (direct memory-get, no envelope)
    let json = r#"{
        "id": "m4", "content": "flat", "memory_type": "episodic", "importance": 0.5,
        "score": 0.6, "created_at": 1000, "last_accessed_at": 1000, "access_count": 0, "tags": []
    }"#;
    let m = deserialize(json);
    assert_eq!(m.score, 0.6);
    assert_eq!(m.smart_score, None);
}
