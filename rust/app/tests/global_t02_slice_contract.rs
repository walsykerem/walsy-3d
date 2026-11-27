use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use png::Decoder;
use serde_json::Value;

const BINARY: &str = env!("CARGO_BIN_EXE_views_to_slices");
const SHEET: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/dancing-dog/sheet.png"
);

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "walsy-slice-contract-{}-{tag}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&Path]) -> Output {
    Command::new(BINARY)
        .args(args)
        .output()
        .expect("slice binary must run")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn png_size(path: &Path) -> (u32, u32) {
    let bytes = fs::read(path).unwrap();
    let decoder = Decoder::new(Cursor::new(bytes));
    let reader = decoder.read_info().unwrap();
    let info = reader.info();
    (info.width, info.height)
}

fn embedded_payload(html: &str) -> Value {
    let marker = "const EMBEDDED = ";
    let at = html.find(marker).expect("viewer payload marker") + marker.len();
    let tail = &html[at..];
    let end = tail.find(";\n").expect("payload terminator");
    serde_json::from_str(&tail[..end]).expect("payload must be JSON")
}

fn corrupt_copy(source: &Path, target: &Path, bytes: &[u8]) {
    fs::copy(source, target).unwrap();
    fs::write(target, bytes).unwrap();
}

#[test]
fn global_t02_carve_counts_and_html_payload() {
    let dir = temp_dir("counts");
    let strip = dir.join("slices.png");
    let html = dir.join("viewer.html");
    let sheet = Path::new(SHEET);

    let out = run(&[sheet, &strip, Path::new("--walsy-html"), &html]);
    assert!(
        out.status.success(),
        "stderr: {}",
        text(&out.stderr)
    );
    assert_eq!(
        text(&out.stdout),
        format!(
            "36 slices of 30 x 65 px -> {}\nviewer -> {}\n",
            strip.display(),
            html.display()
        )
    );
    assert_eq!(png_size(&strip), (1080, 65));

    let page = fs::read_to_string(&html).unwrap();
    let payload = embedded_payload(&page);
    assert_eq!(payload["slices"], 36);
    assert_eq!(payload["sliceWidth"], 30);
    assert_eq!(payload["sliceHeight"], 65);
    assert!(payload["sheet"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn global_t02_repeated_runs_are_byte_identical() {
    let first = temp_dir("repeat-a");
    let second = temp_dir("repeat-b");
    let sheet = Path::new(SHEET);

    for dir in [&first, &second] {
        let strip = dir.join("slices.png");
        let html = dir.join("viewer.html");
        let out = run(&[sheet, &strip, Path::new("--walsy-html"), &html]);
        assert!(out.status.success(), "stderr: {}", text(&out.stderr));
    }
    assert_eq!(
        fs::read(first.join("slices.png")).unwrap(),
        fs::read(second.join("slices.png")).unwrap()
    );
    assert_eq!(
        fs::read(first.join("viewer.html")).unwrap(),
        fs::read(second.join("viewer.html")).unwrap()
    );

    fs::remove_dir_all(&first).unwrap();
    fs::remove_dir_all(&second).unwrap();
}

#[test]
fn global_t02_bad_sheet_and_dimensions_are_rejected() {
    let dir = temp_dir("negative");
    let sheet = Path::new(SHEET);
    let out_png = dir.join("out.png");
    let bad_sheet = dir.join("corrupt-sheet.ppm");
    let wide_sheet = dir.join("wide-sheet.ppm");

    corrupt_copy(sheet, &bad_sheet, b"P6\n1 1\n255\n\x00\x00\x00");
    let bad = run(&[&bad_sheet, &out_png]);
    assert_eq!(bad.status.code(), Some(1));
    assert_eq!(
        text(&bad.stderr),
        "views_to_slices: No foreground drawings found. Check --bg-tol or image alpha.\n"
    );

    corrupt_copy(sheet, &wide_sheet, b"P6\n10001 1\n255\n");
    let wide = run(&[&wide_sheet, &out_png]);
    assert_eq!(wide.status.code(), Some(1));
    assert_eq!(text(&wide.stderr), "views_to_slices: image too large\n");

    let count = run(&[
        sheet,
        &out_png,
        Path::new("--walsy-slices"),
        Path::new("129"),
    ]);
    assert_eq!(count.status.code(), Some(2));
    assert!(text(&count.stderr).ends_with(
        "views_to_slices: error: argument --walsy-slices: out of range: '129' (max 128)\n"
    ));

    fs::remove_dir_all(&dir).unwrap();
}
