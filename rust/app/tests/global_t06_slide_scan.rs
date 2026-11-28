use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const EXPECTED_CONTROL_SEQUENCE: &str = "32.55,29.1,25.65,22.2,18.75,15.3,11.85,8.4,4.95,1.5,34.05,30.6,27.15,23.7,20.25,16.8,13.35,9.9,6.45,3,35.55,32.1,28.65,25.2,21.75,18.3,14.85,11.4,7.95,4.5,1.05,33.6,30.15,26.7,23.25,19.8,16.35,12.9,9.45,6,2.55,35.1,31.65,28.2,24.75,21.3,17.85,14.4,10.95,7.5,4.05,0.6,33.15,29.7,26.25,22.8,19.35,15.9,12.45,9,5.55,2.1,34.65,31.2,27.75,24.3,20.85,17.4,13.95,10.5,7.05,3.6";

fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.ancestors().nth(2).expect("repo root").to_path_buf()
}

fn chrome() -> PathBuf {
    PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
}

struct TempCopy(PathBuf);

impl Drop for TempCopy {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn run_harness(root: &Path, control: bool) -> Value {
    let source = root.join("tests/global/t06-slide-scan.html");
    let _temp;
    let html = if control {
        let copy = source.with_file_name(format!(".t06-control-{}.html", std::process::id()));
        let original = fs::read_to_string(&source).expect("harness source");
        let rival = concat!(
            "<script>window.__t06Rival = function () { const app = window.__t06.app;",
            " const v = app && app.getVol(); if (!v || app.state.mode !== 'scan') return;",
            " window.__t06RivalTicks = (window.__t06RivalTicks || 0) + 1;",
            " app.state.scan = (app.state.scan - 4 + v.n) % v.n; };</script>"
        );
        let injected = original.replacen("<head>", &format!("<head>\n{rival}"), 1);
        fs::write(&copy, injected).expect("write control copy");
        _temp = TempCopy(copy.clone());
        copy
    } else {
        _temp = TempCopy(PathBuf::new());
        source
    };
    let url = format!("file://{}", html.display());
    let output = Command::new(chrome())
        .args([
            "--headless",
            "--disable-gpu",
            "--no-sandbox",
            "--allow-file-access-from-files",
            "--hide-scrollbars",
            "--window-size=900,700",
            "--virtual-time-budget=8000",
            "--dump-dom",
            &url,
        ])
        .output()
        .expect("run chrome harness");
    assert!(
        output.status.success(),
        "chrome exit {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let dom = String::from_utf8_lossy(&output.stdout);
    let tag = "<pre id=\"t06-result\">";
    let start = dom.find(tag).expect("t06-result missing").into();
    let rest = &dom[start..];
    let body_start = rest.find('>').expect("t06-result open tag") + 1;
    let body = &rest[body_start..];
    let end = body.find("</pre>").expect("t06-result close tag");
    let json = &body[..end];
    serde_json::from_str(json).unwrap_or_else(|err| panic!("bad t06 JSON {json}: {err}"))
}

fn sequence(value: &Value) -> String {
    value["scanSequence"]
        .as_array()
        .expect("scanSequence array")
        .iter()
        .map(|v| {
            let n = v.as_f64().expect("numeric scan sample");
            if n.fract() == 0.0 {
                format!("{}", n as i64)
            } else {
                format!("{n}")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn global_t06_slide_scan() {
    let root = repo_root();
    let html = fs::read_to_string(root.join("tests/global/t06-slide-scan.html")).expect("harness source");
    for needle in [
        "my linkedin",
        "https://www.linkedin.com/in/kerem-urhan-225870436/",
        "github star me",
        "https://github.com/walsykerem/walsy-3d",
    ] {
        assert!(html.contains(needle), "harness missing: {needle}");
    }

    let pass = run_harness(&root, false);
    println!("PASS JSON: {pass}");
    assert_eq!(pass["test"], "t06-slide-scan");
    assert_eq!(pass["control"], false);
    assert_eq!(pass["exit"], 0, "positive run failed: {}", pass["violations"]);
    assert_eq!(pass["slices"], 36);
    assert_eq!(pass["frames"], 72);
    assert_eq!(pass["backwardSteps"], 0);
    assert_eq!(pass["zoomJumps"], 0);
    assert_eq!(pass["modeJumps"], 0);
    assert_eq!(pass["repeatSequence"], pass["scanSequence"]);
    assert_eq!(sequence(&pass).split(',').count(), 72);

    let fail = run_harness(&root, true);
    println!("CONTROL JSON: {fail}");
    assert_eq!(fail["control"], true);
    assert_eq!(fail["exit"], 1, "negative control did not fail: {}", fail["violations"]);
    assert!(fail["backwardSteps"].as_u64().unwrap_or(0) > 0);
    let violations = fail["violations"].as_array().expect("violations array");
    assert!(
        violations.iter().any(|v| v.as_str().unwrap_or("").contains("non-monotonic")),
        "missing non-monotonic violation: {violations:?}"
    );
    let observed = sequence(&fail);
    println!("CONTROL SEQUENCE: {observed}");
    assert_eq!(observed, EXPECTED_CONTROL_SEQUENCE);
}
