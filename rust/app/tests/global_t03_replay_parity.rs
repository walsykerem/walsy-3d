//! Global parity for the shipped dancing-dog example.
//! Sheet -> strip, normal map, emissive mask, RGB preview, viewer payload.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_views_to_slices");
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

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn fresh_dir(tag: &str) -> PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("walsy-parity-{tag}-{ns}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn simulate(dir: &Path) {
    let status = Command::new(BIN)
        .arg(root().join("examples/dancing-dog/sheet.png"))
        .arg(dir.join("slices.png"))
        .arg("--walsy-normals")
        .arg(dir.join("normals.png"))
        .arg("--walsy-emissive")
        .arg(dir.join("emissive.png"))
        .arg("--walsy-preview")
        .arg(dir.join("check.png"))
        .arg("--walsy-html")
        .arg(dir.join("viewer.html"))
        .status()
        .expect("run views_to_slices");
    assert!(status.success(), "views_to_slices failed");
}

// Signature, chunk length, "IHDR", width, height, bit depth, colour type.
fn ihdr(bytes: &[u8]) -> Result<(u32, u32, u8), String> {
    if bytes.len() < 26 {
        return Err("truncated PNG header".into());
    }
    if bytes[..8] != *b"\x89PNG\r\n\x1a\n" {
        return Err("bad PNG signature".into());
    }
    if bytes[8..12] != 13u32.to_be_bytes() || bytes[12..16] != *b"IHDR" {
        return Err("bad IHDR chunk".into());
    }
    Ok((
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
        bytes[25],
    ))
}

fn assert_rgb_png(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let (_, _, colour) = ihdr(&bytes)?;
    if colour != 2 {
        return Err(format!("expected colour type 2, got colour type {colour}"));
    }
    Ok(())
}

fn same_bytes(actual: &Path, expected: &Path) {
    let a = fs::read(actual).unwrap();
    let b = fs::read(expected).unwrap();
    assert_eq!(
        a, b,
        "{} differs from {}",
        actual.display(),
        expected.display()
    );
}

#[test]
fn sheet_reproduces_all_outputs() {
    let dir = fresh_dir("parity");
    simulate(&dir);

    let example = root().join("examples/dancing-dog");
    let snaps = root().join("rust/app/tests/fixtures/dancing-dog");
    same_bytes(&dir.join("slices.png"), &example.join("slices.png"));
    same_bytes(&dir.join("check.png"), &example.join("check.png"));
    same_bytes(&dir.join("normals.png"), &snaps.join("normals.png"));
    same_bytes(&dir.join("emissive.png"), &snaps.join("emissive.png"));

    let (w, h, colour) = ihdr(&fs::read(dir.join("check.png")).unwrap()).unwrap();
    assert!(w > 0 && h > 0);
    assert_eq!(colour, 2);
    assert_rgb_png(&dir.join("check.png")).unwrap();

    let html = fs::read_to_string(dir.join("viewer.html")).unwrap();
    let line = html
        .lines()
        .find(|l| l.starts_with("const EMBEDDED = "))
        .expect("embedded payload line");
    let json = line
        .trim_start_matches("const EMBEDDED = ")
        .trim_end_matches(';');
    let payload: Value = serde_json::from_str(json).expect("payload JSON");
    let obj = payload.as_object().expect("payload object");
    assert_eq!(
        obj.len(),
        9,
        "payload keys: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
    for key in KEYS {
        assert!(obj.contains_key(key), "missing payload key {key}");
    }

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rgba_check_is_rejected_with_colour_type_six() {
    let dir = fresh_dir("negative");
    simulate(&dir);

    // The carved strip is RGBA; it cannot stand in for the RGB preview.
    fs::copy(dir.join("slices.png"), dir.join("check.png")).unwrap();
    let (_, _, colour) = ihdr(&fs::read(dir.join("check.png")).unwrap()).unwrap();
    assert_eq!(colour, 6, "substituted image must be RGBA");
    let err = assert_rgb_png(&dir.join("check.png")).unwrap_err();
    assert!(err.contains("colour type 6"), "unexpected failure: {err}");

    fs::remove_dir_all(&dir).unwrap();
}
