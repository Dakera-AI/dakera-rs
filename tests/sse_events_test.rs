//! Tests for SSE event type deserialization — Connected event (DAK-720, v0.8.3).
//!
//! Verifies that `DakeraEvent::Connected` and `MemoryEvent` connected handshake
//! events deserialize correctly from the JSON payloads the server sends.

use dakera_client::{DakeraEvent, MemoryEvent};

// ============================================================================
// DakeraEvent::Connected deserialization
// ============================================================================

#[test]
fn test_dakera_event_connected_deserializes_from_json() {
    let json = r#"{"type":"connected","timestamp":1700000000000}"#;
    let event: DakeraEvent = serde_json::from_str(json).unwrap();
    match event {
        DakeraEvent::Connected { timestamp } => {
            assert_eq!(timestamp, 1700000000000);
        }
        other => panic!("Expected Connected variant, got {:?}", other),
    }
}

#[test]
fn test_dakera_event_connected_event_type_str() {
    let event = DakeraEvent::Connected {
        timestamp: 1700000000000,
    };
    assert_eq!(event.event_type(), "connected");
}

#[test]
fn test_dakera_event_connected_round_trips() {
    let event = DakeraEvent::Connected {
        timestamp: 1774296453000,
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: DakeraEvent = serde_json::from_str(&json).unwrap();
    match parsed {
        DakeraEvent::Connected { timestamp } => assert_eq!(timestamp, 1774296453000),
        other => panic!("Round-trip produced wrong variant: {:?}", other),
    }
}

// ============================================================================
// MemoryEvent connected handshake deserialization
// ============================================================================

#[test]
fn test_memory_event_connected_maps_type_to_event_type() {
    // Server sends {"type":"connected","timestamp":...} for the handshake.
    // The `type` alias on event_type must absorb this key.
    let json = r#"{"type":"connected","timestamp":1700000000000}"#;
    let event: MemoryEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type, "connected");
}

#[test]
fn test_memory_event_connected_defaults_agent_id_to_empty() {
    let json = r#"{"type":"connected","timestamp":1700000000000}"#;
    let event: MemoryEvent = serde_json::from_str(json).unwrap();
    // agent_id has #[serde(default)] so it must be "" when absent.
    assert_eq!(event.agent_id, "");
}

#[test]
fn test_memory_event_connected_preserves_timestamp() {
    let ts: u64 = 1774296453999;
    let json = format!(r#"{{"type":"connected","timestamp":{}}}"#, ts);
    let event: MemoryEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event.timestamp, ts);
}

#[test]
fn test_memory_event_connected_has_no_optional_fields() {
    let json = r#"{"type":"connected","timestamp":1700000000000}"#;
    let event: MemoryEvent = serde_json::from_str(json).unwrap();
    assert!(event.memory_id.is_none());
    assert!(event.content.is_none());
    assert!(event.importance.is_none());
    assert!(event.tags.is_none());
    assert!(event.session_id.is_none());
}

#[test]
fn test_memory_event_regular_stored_event_unaffected() {
    let json = r#"{
        "event_type": "stored",
        "agent_id": "qa",
        "timestamp": 1700000000000,
        "memory_id": "mem_abc",
        "content": "test memory",
        "importance": 0.8
    }"#;
    let event: MemoryEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type, "stored");
    assert_eq!(event.agent_id, "qa");
    assert_eq!(event.memory_id.as_deref(), Some("mem_abc"));
    assert!((event.importance.unwrap() - 0.8).abs() < 1e-5);
}
