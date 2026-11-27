use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PAIRS: [(&str, &str); 2] = [
    ("my linkedin", "https://www.linkedin.com/in/kerem-urhan-225870436/"),
    ("github star me", "https://github.com/walsykerem/walsy-3d"),
];
const MARKER: &str = "/*__EMBEDDED_STACK__*/null";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        let ty = entry.file_type().unwrap();
        if ty.is_dir() {
            walk(&entry.path(), out);
        } else if ty.is_file() && entry.path().extension().is_some_and(|e| e == "html") {
            out.push(entry.path());
        }
    }
}

fn html_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn check(path: &Path, text: &str) -> Result<(), String> {
    for (label, url) in PAIRS {
        let exact = format!("{label} → {url}");
        if !text.contains(&exact) {
            return Err(format!("{}: missing {url} for {label}", path.display()));
        }
    }
    Ok(())
}

#[test]
fn every_shipped_html_carries_both_exact_link_pairs() {
    let files = html_files(&root());
    assert!(!files.is_empty(), "no shipped HTML found");
    let errors: Vec<String> = files
        .iter()
        .filter_map(|p| check(p, &read(p)).err())
        .collect();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn harness_html_is_shipped_and_carries_both_exact_link_pairs() {
    let dir = root().join("tests/global");
    let files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "html"))
        .collect();
    assert!(!files.is_empty(), "no harness HTML under {}", dir.display());
    for path in files {
        check(&path, &read(&path)).unwrap_or_else(|e| panic!("{e}"));
    }
}

#[test]
fn viewer_template_keeps_payload_marker_and_both_link_pairs() {
    let path = root().join("rust/scripts-rs/src/viewer.html");
    let text = read(&path);
    assert!(text.contains(MARKER), "{}: payload marker changed", path.display());
    check(&path, &text).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn negative_control_html_missing_one_url_fails_with_path_and_url() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "walsy-t08-{}-{nanos}.html",
        std::process::id()
    ));
    let (label, url) = PAIRS[0];
    fs::write(&path, format!("<!-- {label} → {url} -->")).unwrap();
    let error = check(&path, &read(&path)).unwrap_err();
    assert!(error.contains(path.to_str().unwrap()), "{error}");
    assert!(error.contains(PAIRS[1].1), "{error}");
    fs::remove_file(&path).unwrap();
}
