//! Every endpoint the SDK calls must exist in the Dakera v0.12.0 router.
//!
//! `v012_routes.txt` is the route table of the server's `crates/api/src/lib.rs`
//! (method and path, `{x}` placeholders normalised to `{}`; the `/admin/*`
//! routes also under `/v1/admin/*`, the key router under `/admin/keys`).  The
//! test reads the SDK sources, finds every `"{}/path"` URL template and checks
//! its path against that table.  A call to a route the server does not serve
//! (it would 404 on every server) fails the build.

use std::collections::HashSet;
use std::fs;

fn server_paths() -> HashSet<String> {
    include_str!("v012_routes.txt")
        .lines()
        .filter_map(|l| l.split_once(' ').map(|(_, p)| p.trim().to_string()))
        .collect()
}

/// Every `"{}/..."` URL template in `src/*.rs`, with its file and path part.
fn sdk_paths() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            let Some(start) = line.find("\"{}/") else {
                continue;
            };
            let rest = &line[start + 3..];
            let Some(end) = rest.find('"') else {
                continue;
            };
            let template = &rest[..end];
            let template = template.split('?').next().unwrap();
            out.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                template.trim_end_matches('/').to_string(),
            ));
        }
    }
    out
}

#[test]
fn every_sdk_endpoint_exists_in_the_v012_router() {
    let server = server_paths();
    assert!(server.len() > 150, "route table looks empty");
    let calls = sdk_paths();
    assert!(calls.len() > 150, "found only {} SDK URLs", calls.len());
    let mut missing = Vec::new();
    for (file, path) in &calls {
        // The dakera-ode sidecar is a separate service.
        if path.starts_with("/ode") {
            continue;
        }
        if !server.contains(path) {
            missing.push(format!("{file}: {path}"));
        }
    }
    assert!(
        missing.is_empty(),
        "SDK calls routes the v0.12.0 server does not serve:\n{}",
        missing.join("\n")
    );
}

#[test]
fn the_routes_this_release_depends_on_are_in_the_table() {
    let server = server_paths();
    for p in [
        "/health/ready",
        "/health/live",
        "/v1/capabilities",
        "/v1/namespaces/{}/attachments",
        "/v1/namespaces/{}/attachments/{}/transcribe/{}",
        "/v1/namespaces/{}/attachments/{}/index/{}",
        "/v1/namespaces/{}/records",
        "/v1/namespaces/{}/records/{}",
        "/v1/namespaces/{}/config",
        "/v1/memory/update/{}",
        "/v1/memory/feedback",
        "/v1/audit/export",
        "/v1/admin/quotas/{}",
        "/v1/admin/quotas/default",
        "/v1/admin/indexes/stats",
    ] {
        assert!(server.contains(p), "{p} missing from the route table");
    }
}
