/// Typed filter builder helpers for the Dakera metadata filter DSL.
///
/// All functions return `serde_json::Value` so they compose directly with
/// `with_filter(...)` on any request builder.
///
/// # Examples
///
/// ```no_run
/// use dakera_client::filter as F;
/// use serde_json::json;
///
/// // Recall memories tagged for a specific person (CE-79 array operator)
/// let f = json!({"tags": F::array_contains("entity:PERSON:alice")});
///
/// // Logical combinator
/// let f = F::and([
///     json!({"importance": F::gte(0.8)}),
///     json!({"tags": F::array_contains("entity:PERSON:alice")}),
/// ]);
/// ```
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Comparison operators
// ---------------------------------------------------------------------------

/// `$eq` — equal to `value`.
pub fn eq(value: impl Into<Value>) -> Value {
    json!({"$eq": value.into()})
}

/// `$ne` — not equal to `value`.
pub fn ne(value: impl Into<Value>) -> Value {
    json!({"$ne": value.into()})
}

/// `$gt` — greater than `value`.
pub fn gt(value: impl Into<Value>) -> Value {
    json!({"$gt": value.into()})
}

/// `$gte` — greater than or equal to `value`.
pub fn gte(value: impl Into<Value>) -> Value {
    json!({"$gte": value.into()})
}

/// `$lt` — less than `value`.
pub fn lt(value: impl Into<Value>) -> Value {
    json!({"$lt": value.into()})
}

/// `$lte` — less than or equal to `value`.
pub fn lte(value: impl Into<Value>) -> Value {
    json!({"$lte": value.into()})
}

/// `$in` — field value is in the given list.
pub fn in_<I, V>(values: I) -> Value
where
    I: IntoIterator<Item = V>,
    V: Into<Value>,
{
    let arr: Vec<Value> = values.into_iter().map(Into::into).collect();
    json!({"$in": arr})
}

/// `$nin` — field value is NOT in the given list.
pub fn nin<I, V>(values: I) -> Value
where
    I: IntoIterator<Item = V>,
    V: Into<Value>,
{
    let arr: Vec<Value> = values.into_iter().map(Into::into).collect();
    json!({"$nin": arr})
}

/// `$exists` — field presence check.
pub fn exists(present: bool) -> Value {
    json!({"$exists": present})
}

// ---------------------------------------------------------------------------
// String operators
// ---------------------------------------------------------------------------

/// `$contains` — case-sensitive substring match.
pub fn contains(substr: &str) -> Value {
    json!({"$contains": substr})
}

/// `$icontains` — case-insensitive substring match.
pub fn icontains(substr: &str) -> Value {
    json!({"$icontains": substr})
}

/// `$startsWith` — prefix match.
pub fn starts_with(prefix: &str) -> Value {
    json!({"$startsWith": prefix})
}

/// `$endsWith` — suffix match.
pub fn ends_with(suffix: &str) -> Value {
    json!({"$endsWith": suffix})
}

/// `$glob` — glob pattern match (supports `*` and `?` wildcards).
pub fn glob(pattern: &str) -> Value {
    json!({"$glob": pattern})
}

/// `$regex` — regular expression match.
pub fn regex(pattern: &str) -> Value {
    json!({"$regex": pattern})
}

// ---------------------------------------------------------------------------
// Array operators (CE-79)
// ---------------------------------------------------------------------------

/// `$arrayContains` — the metadata array field contains `value`.
///
/// Primary use case: entity-scoped vector search via server-assigned tags
/// (e.g. `entity:PERSON:alice`). Enables HNSW pre-filtering to a single
/// entity's memories before semantic ranking.
pub fn array_contains(value: impl Into<Value>) -> Value {
    json!({"$arrayContains": value.into()})
}

/// `$arrayContainsAll` — the metadata array field contains ALL of `values`.
pub fn array_contains_all<I, V>(values: I) -> Value
where
    I: IntoIterator<Item = V>,
    V: Into<Value>,
{
    let arr: Vec<Value> = values.into_iter().map(Into::into).collect();
    json!({"$arrayContainsAll": arr})
}

/// `$arrayContainsAny` — the metadata array field contains ANY of `values`.
pub fn array_contains_any<I, V>(values: I) -> Value
where
    I: IntoIterator<Item = V>,
    V: Into<Value>,
{
    let arr: Vec<Value> = values.into_iter().map(Into::into).collect();
    json!({"$arrayContainsAny": arr})
}

// ---------------------------------------------------------------------------
// Logical combinators
// ---------------------------------------------------------------------------

/// `$and` — all conditions must match.
pub fn and<I>(conditions: I) -> Value
where
    I: IntoIterator<Item = Value>,
{
    let arr: Vec<Value> = conditions.into_iter().collect();
    json!({"$and": arr})
}

/// `$or` — at least one condition must match.
pub fn or<I>(conditions: I) -> Value
where
    I: IntoIterator<Item = Value>,
{
    let arr: Vec<Value> = conditions.into_iter().collect();
    json!({"$or": arr})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_comparison_ops() {
        assert_eq!(eq("hello"), json!({"$eq": "hello"}));
        assert_eq!(gte(0.8_f64), json!({"$gte": 0.8}));
        assert_eq!(lt(100_i64), json!({"$lt": 100}));
    }

    #[test]
    fn test_string_ops() {
        assert_eq!(contains("alice"), json!({"$contains": "alice"}));
        assert_eq!(icontains("Alice"), json!({"$icontains": "Alice"}));
        assert_eq!(starts_with("entity:"), json!({"$startsWith": "entity:"}));
        assert_eq!(ends_with(":alice"), json!({"$endsWith": ":alice"}));
        assert_eq!(glob("entity:*:alice"), json!({"$glob": "entity:*:alice"}));
        assert_eq!(
            regex("^entity:PERSON:"),
            json!({"$regex": "^entity:PERSON:"})
        );
    }

    #[test]
    fn test_array_ops() {
        assert_eq!(
            array_contains("entity:PERSON:alice"),
            json!({"$arrayContains": "entity:PERSON:alice"})
        );
        assert_eq!(
            array_contains_all(["entity:PERSON:alice", "entity:PERSON:bob"]),
            json!({"$arrayContainsAll": ["entity:PERSON:alice", "entity:PERSON:bob"]})
        );
        assert_eq!(
            array_contains_any(["entity:PERSON:alice", "entity:PERSON:carol"]),
            json!({"$arrayContainsAny": ["entity:PERSON:alice", "entity:PERSON:carol"]})
        );
    }

    #[test]
    fn test_logical_ops() {
        let f = and([
            json!({"importance": gte(0.8_f64)}),
            json!({"tags": array_contains("entity:PERSON:alice")}),
        ]);
        assert_eq!(
            f,
            json!({"$and": [
                {"importance": {"$gte": 0.8}},
                {"tags": {"$arrayContains": "entity:PERSON:alice"}}
            ]})
        );
    }

    #[test]
    fn test_in_nin() {
        assert_eq!(in_(["a", "b", "c"]), json!({"$in": ["a", "b", "c"]}));
        assert_eq!(nin([1_i64, 2_i64, 3_i64]), json!({"$nin": [1, 2, 3]}));
    }

    #[test]
    fn test_exists() {
        assert_eq!(exists(true), json!({"$exists": true}));
        assert_eq!(exists(false), json!({"$exists": false}));
    }
}
