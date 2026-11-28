use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PASS: &str = r#"{"test":"t04-turntable","ok":true,"frames":19,"rafLoops":1,"checks":4}"#;
const NEGATIVE: &str = r#"{"test":"t04-turntable","ok":false,"reason":"raf_loops"}"#;
const LINKEDIN: &str = "my linkedin → https://www.linkedin.com/in/kerem-urhan-225870436/";
const GITHUB: &str = "github star me → https://github.com/walsykerem/walsy-3d";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

fn harness() -> PathBuf {
    root().join("tests/global/t04-turntable.html")
}

fn assert_links() {
    let source = fs::read_to_string(harness()).expect("harness readable");
    assert!(source.contains(LINKEDIN), "missing exact label and URL: {LINKEDIN}");
    assert!(source.contains(GITHUB), "missing exact label and URL: {GITHUB}");
}

fn node_ready() -> bool {
    match Command::new("node").arg("--version").output() {
        Ok(out) if out.status.success() => true,
        Ok(out) => panic!("node --version failed: {}", String::from_utf8_lossy(&out.stderr)),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            eprintln!("BLOCKER: node unavailable; t04-turntable harness not executed");
            false
        }
        Err(err) => panic!("node --version error: {err}"),
    }
}

#[test]
fn global_t04_turntable_harness_passes() {
    assert_links();
    if !node_ready() {
        return;
    }
    let out = Command::new("node")
        .arg(harness())
        .current_dir(root())
        .output()
        .expect("run t04 harness");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "harness nonzero: stdout={stdout} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(stdout.trim(), PASS);
}

#[test]
fn global_t04_turntable_duplicate_raf_negative_control_fails() {
    assert_links();
    if !node_ready() {
        return;
    }
    let root = root();
    let viewer = root.join("examples/dancing-dog/viewer.html");
    let source = fs::read_to_string(&viewer).expect("shipped viewer readable");
    let needle = "requestAnimationFrame(walsyTick);\n</script>";
    assert!(source.contains(needle), "shipped rAF scheduler marker missing");
    let mutated = source.replacen(
        needle,
        "requestAnimationFrame(walsyTick);\nrequestAnimationFrame(walsyTick);\n</script>",
        1,
    );
    assert_ne!(mutated, source, "negative control did not mutate viewer");

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("walsy-t04-neg-{}-{unique}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp control dir");
    let copy = dir.join("viewer.html");
    fs::write(&copy, mutated).expect("temp control viewer");
    let out = Command::new("node")
        .arg(harness())
        .arg(&copy)
        .current_dir(&root)
        .output()
        .expect("run negative control");
    let status = out.status;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    fs::remove_dir_all(&dir).ok();

    assert!(!status.success(), "duplicate scheduler unexpectedly passed: {stdout}");
    assert_eq!(stdout, NEGATIVE, "stderr={stderr}");
}
