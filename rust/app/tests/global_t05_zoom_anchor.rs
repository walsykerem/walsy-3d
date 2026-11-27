use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const LINKEDIN: &str = "my linkedin: https://www.linkedin.com/in/kerem-urhan-225870436/";
const GITHUB: &str = "github star me: https://github.com/walsykerem/walsy-3d";
const ANCHOR: &str = "        state.cx = ax + (view.width / 2 - px) / k1;\n        state.cy = ay + (view.height / 2 - py) / k1;\n";
const FRAME_SRC: &str = "src=\"../../examples/dancing-dog/dancing-dog.html\"";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn chrome_path() -> PathBuf {
    if let Ok(p) = std::env::var("CHROME_PATH") {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }
    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return p;
        }
    }
    panic!("no Chrome binary found; set CHROME_PATH");
}

fn as_bool(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn as_f64(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

fn file_url(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

fn run_harness(html_path: &Path, profile: &Path) -> Value {
    let mut child = Command::new(chrome_path())
        .args([
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            "--no-first-run",
            "--no-default-browser-check",
            "--allow-file-access-from-files",
            "--virtual-time-budget=4000",
            "--window-size=900,700",
            "--dump-dom",
        ])
        .arg(format!("--user-data-dir={}", profile.to_string_lossy()))
        .arg(file_url(html_path))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn Chrome");
    let mut dumped = None;
    if let Some(stdout) = child.stdout.take() {
        let open = "<pre id=\"result\">";
        for line in BufReader::new(stdout).lines() {
            let line = line.expect("read Chrome stdout");
            if let Some(i) = line.find(open) {
                let tail = &line[i + open.len()..];
                let end = tail.find("</pre>").expect("result end tag");
                dumped = Some(tail[..end].to_string());
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let text = dumped.expect("harness did not emit <pre id=\"result\">");
    serde_json::from_str(&text).expect("harness result is not JSON")
}

#[test]
fn global_t05_zoom_anchor() {
    let root = repo_root();
    let harness = root.join("tests/global/t05-zoom-anchor.html");
    let source = fs::read_to_string(&harness).expect("read harness");
    assert!(source.contains(LINKEDIN), "missing exact linkedin pair");
    assert!(source.contains(GITHUB), "missing exact github pair");
    assert!(source.contains(ANCHOR), "missing pointer-anchor block");

    let tmp = std::env::temp_dir().join(format!("walsy-global-t05-{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp);

    let positive = run_harness(&harness, &tmp.join("positive"));
    assert_eq!(positive.get("error").and_then(Value::as_str), None);
    assert!(as_bool(&positive, "shipped_loaded"), "shipped viewer not loaded");
    assert!(as_bool(&positive, "off_center"), "cursor must not be center");
    assert!(as_bool(&positive, "zoom_changed"), "wheel did not change zoom");
    assert_eq!(positive.get("events").and_then(Value::as_u64), Some(8));
    assert!(as_bool(&positive, "link_pairs"), "link pairs missing in harness");
    assert!(!as_bool(&positive, "negative"));
    assert!(as_bool(&positive, "clamp_ok"));
    assert_eq!(as_f64(&positive, "clamp_high"), 8.0);
    assert_eq!(as_f64(&positive, "clamp_low"), 0.5);
    let drift = as_f64(&positive, "max_drift_px");
    assert!(drift <= 1.0, "pointer-local anchor drift {drift}px > 1px");
    assert!(as_bool(&positive, "ok"), "positive result not ok: {positive}");
    eprintln!("positive max drift: {drift}px");

    // Negative control: temp copy swaps the pointer re-anchor for center zoom.
    let shipped = file_url(&root.join("examples/dancing-dog/dancing-dog.html"));
    let negative_html = source
        .replace("const NEGATIVE = false;", "const NEGATIVE = true;")
        .replace(ANCHOR, "        state.cx = state.cx; // center zoom\n        state.cy = state.cy; // center zoom\n")
        .replace(FRAME_SRC, &format!("src=\"{shipped}\""));
    assert_ne!(negative_html, source, "negative control replacement failed");
    let negative_path = tmp.join("t05-zoom-anchor-negative.html");
    fs::write(&negative_path, &negative_html).expect("write negative copy");
    let negative = run_harness(&negative_path, &tmp.join("negative"));
    assert!(as_bool(&negative, "negative"), "negative flag missing");
    assert!(as_bool(&negative, "zoom_changed"), "negative zoom did not change");
    let neg_drift = as_f64(&negative, "max_drift_px");
    assert!(neg_drift > 1.0, "negative control did not drift: {neg_drift}px");
    assert!(!as_bool(&negative, "ok"), "negative control unexpectedly passed");
    eprintln!("negative control max drift: {neg_drift}px (expected failure)");

    let _ = fs::remove_dir_all(&tmp);
}
