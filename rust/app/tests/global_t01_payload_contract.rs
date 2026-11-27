use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

const KEYS: [&str; 9] = [
    "title",
    "slices",
    "sliceWidth",
    "sliceHeight",
    "sheet",
    "top",
    "side",
    "front",
    "back",
];

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("walsy-global-t01-{tag}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn generate_viewer(dir: &PathBuf) -> String {
    let out = dir.join("slices.png");
    let html_path = dir.join("viewer.html");
    let status = Command::new(env!("CARGO_BIN_EXE_views_to_slices"))
        .arg("--walsy-compose")
        .arg("apple")
        .arg(&out)
        .arg("--walsy-html")
        .arg(&html_path)
        .status()
        .expect("run generator");
    assert!(status.success(), "generator exit {status}");
    assert!(out.is_file(), "strip artifact missing");
    fs::read_to_string(&html_path).expect("read generated viewer")
}

fn embedded(html: &str) -> Value {
    let marker = "const EMBEDDED = ";
    let start = html
        .find(marker)
        .expect("viewer payload assignment missing")
        + marker.len();
    let end = start
        + html[start..]
            .find(";\n")
            .expect("viewer payload terminator missing");
    serde_json::from_str(&html[start..end]).expect("viewer payload JSON invalid")
}

fn contract(html: &str) -> Result<(), String> {
    let payload = embedded(html);
    let map = payload
        .as_object()
        .ok_or_else(|| "payload not object".to_owned())?;

    let expected: BTreeSet<&str> = KEYS.into_iter().collect();
    let actual: BTreeSet<&str> = map.keys().map(String::as_str).collect();
    let missing: Vec<&str> = expected.difference(&actual).copied().collect();
    let extra: Vec<&str> = actual.difference(&expected).copied().collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(format!(
            "payload key mismatch: missing={} extra={}",
            missing.join(","),
            extra.join(",")
        ));
    }

    if !html.contains("const data = EMBEDDED || {};") {
        return Err("viewer payload wiring missing".to_owned());
    }

    let title = map["title"].as_str().ok_or("viewer title not string")?;
    if title.is_empty() {
        return Err("viewer title empty".to_owned());
    }
    let _slices = map["slices"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("viewer slices not positive")?;
    let _width = map["sliceWidth"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("viewer sliceWidth not positive")?;
    let _height = map["sliceHeight"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("viewer sliceHeight not positive")?;
    for key in ["sheet", "top", "side", "front", "back"] {
        let uri = map[key]
            .as_str()
            .ok_or_else(|| format!("viewer {key} not string"))?;
        if !uri.starts_with("data:image/png;base64,") {
            return Err(format!("viewer {key} not png data"));
        }
    }
    Ok(())
}

#[test]
fn global_t01_generated_viewer_payload_contract() {
    let dir = temp_dir("pass");
    let html = generate_viewer(&dir);
    contract(&html).expect("generated viewer contract");
    fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn global_t01_negative_control_renamed_key() {
    let dir = temp_dir("negative");
    let html = generate_viewer(&dir);
    assert!(contract(&html).is_ok(), "fixture must start valid");

    let patched = html.replacen("\"back\":", "\"rear\":", 1);
    assert_ne!(patched, html, "rename did not apply");
    let copy = dir.join("viewer-renamed.html");
    fs::write(&copy, &patched).expect("write patched copy");
    let patched_html = fs::read_to_string(&copy).expect("read patched copy");

    assert_eq!(
        contract(&patched_html).unwrap_err(),
        "payload key mismatch: missing=back extra=rear"
    );
    fs::remove_dir_all(&dir).expect("cleanup");
}
