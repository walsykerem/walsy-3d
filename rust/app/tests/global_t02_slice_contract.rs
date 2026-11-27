use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use png::{Decoder, Transformations};
use serde_json::{Value, json};

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

fn sheet_value(path: &Path) -> Value {
    let bytes = fs::read(path).unwrap();
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let (w, h) = {
        let info = reader.info();
        (info.width as usize, info.height as usize)
    };
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut buf).unwrap();
    let samples = reader.info().color_type.samples();
    let mut rows = Vec::with_capacity(h);
    for y in 0..h {
        let mut row = Vec::with_capacity(w);
        for x in 0..w {
            let at = (y * w + x) * samples;
            row.push(json!([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]));
        }
        rows.push(Value::Array(row));
    }
    Value::Array(rows)
}

#[test]
fn global_t02_crate_engine_counts() {
    let sheet = sheet_value(Path::new(SHEET));
    let carve = || {
        scripts_rs::engine::views_to_slices(
            sheet.clone(),
            Some(36),
            "auto".into(),
            "left".into(),
            "auto".into(),
            24,
            0.2,
            70,
            28,
            false,
        )
        .unwrap()
    };
    let first = carve();
    let second = carve();
    assert_eq!(first.to_string(), second.to_string());

    let planes = first[0].as_array().unwrap();
    assert_eq!(planes.len(), 36);
    for plane in planes {
        let rows = plane.as_array().unwrap();
        assert_eq!(rows.len(), 65);
        for row in rows {
            assert_eq!(row.as_array().unwrap().len(), 30);
        }
    }
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
    assert!(!out_png.exists());

    corrupt_copy(sheet, &wide_sheet, b"P6\n10001 1\n255\n");
    let wide = run(&[&wide_sheet, &out_png]);
    assert_eq!(wide.status.code(), Some(1));
    assert_eq!(text(&wide.stderr), "views_to_slices: image too large\n");
    assert!(!out_png.exists());

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
