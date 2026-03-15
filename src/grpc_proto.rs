//! gRPC Protocol Messages for Dakera
//!
//! Mirrors the server-side protobuf definitions for client-server communication.

#![cfg(feature = "grpc")]

use prost::Message;

/// Vector with optional metadata
#[derive(Clone, PartialEq, Message)]
pub struct ProtoVector {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(float, repeated, tag = "2")]
    pub values: Vec<f32>,
    #[prost(string, optional, tag = "3")]
    pub metadata_json: Option<String>,
}

/// Search result
#[derive(Clone, PartialEq, Message)]
pub struct ProtoSearchResult {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(float, tag = "2")]
    pub score: f32,
    #[prost(string, optional, tag = "3")]
    pub metadata_json: Option<String>,
    #[prost(float, repeated, tag = "4")]
    pub values: Vec<f32>,
}

// Health
#[derive(Clone, PartialEq, Message)]
pub struct HealthRequest {}

#[derive(Clone, PartialEq, Message)]
pub struct HealthResponse {
    #[prost(string, tag = "1")]
    pub status: String,
    #[prost(string, tag = "2")]
    pub version: String,
}

// Namespaces
#[derive(Clone, PartialEq, Message)]
pub struct GetNamespaceRequest {
    #[prost(string, tag = "1")]
    pub namespace: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct NamespaceInfo {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(uint64, tag = "2")]
    pub vector_count: u64,
    #[prost(uint32, optional, tag = "3")]
    pub dimension: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteNamespaceRequest {
    #[prost(string, tag = "1")]
    pub namespace: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteNamespaceResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
}

// Vector operations
#[derive(Clone, PartialEq, Message)]
pub struct GrpcUpsertRequest {
    #[prost(string, tag = "1")]
    pub namespace: String,
    #[prost(message, repeated, tag = "2")]
    pub vectors: Vec<ProtoVector>,
}

#[derive(Clone, PartialEq, Message)]
pub struct UpsertResponse {
    #[prost(uint64, tag = "1")]
    pub upserted_count: u64,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrpcQueryRequest {
    #[prost(string, tag = "1")]
    pub namespace: String,
    #[prost(float, repeated, tag = "2")]
    pub vector: Vec<f32>,
    #[prost(uint32, tag = "3")]
    pub top_k: u32,
    #[prost(string, tag = "4")]
    pub distance_metric: String,
    #[prost(bool, tag = "5")]
    pub include_metadata: bool,
    #[prost(bool, tag = "6")]
    pub include_vectors: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct QueryResponse {
    #[prost(message, repeated, tag = "1")]
    pub results: Vec<ProtoSearchResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteVectorsRequest {
    #[prost(string, tag = "1")]
    pub namespace: String,
    #[prost(string, repeated, tag = "2")]
    pub ids: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteVectorsResponse {
    #[prost(uint64, tag = "1")]
    pub deleted_count: u64,
}

// Cache warming
#[derive(Clone, PartialEq, Message)]
pub struct WarmCacheRequest {
    #[prost(string, tag = "1")]
    pub namespace: String,
    #[prost(string, repeated, tag = "2")]
    pub vector_ids: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WarmCacheResponse {
    #[prost(uint64, tag = "1")]
    pub warmed_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_vector_encode_decode() {
        let vec = ProtoVector {
            id: "test".to_string(),
            values: vec![1.0, 2.0, 3.0],
            metadata_json: Some(r#"{"key":"value"}"#.to_string()),
        };

        let encoded = vec.encode_to_vec();
        let decoded = ProtoVector::decode(encoded.as_slice()).unwrap();

        assert_eq!(vec.id, decoded.id);
        assert_eq!(vec.values, decoded.values);
        assert_eq!(vec.metadata_json, decoded.metadata_json);
    }

    #[test]
    fn test_health_response() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            version: "0.1.0".to_string(),
        };

        let encoded = response.encode_to_vec();
        let decoded = HealthResponse::decode(encoded.as_slice()).unwrap();

        assert_eq!(response.status, decoded.status);
        assert_eq!(response.version, decoded.version);
    }

    #[test]
    fn test_query_request() {
        let req = GrpcQueryRequest {
            namespace: "test".to_string(),
            vector: vec![1.0, 2.0, 3.0],
            top_k: 10,
            distance_metric: "cosine".to_string(),
            include_metadata: true,
            include_vectors: false,
        };

        let encoded = req.encode_to_vec();
        let decoded = GrpcQueryRequest::decode(encoded.as_slice()).unwrap();

        assert_eq!(req.namespace, decoded.namespace);
        assert_eq!(req.vector, decoded.vector);
        assert_eq!(req.top_k, decoded.top_k);
    }
}
