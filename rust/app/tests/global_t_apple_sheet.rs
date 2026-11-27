//! Four-view apple sheet gates for walsy-3d. Primitives moved with fast-anything; CLI discovery used cli-hub.
//! Source credit: Kerem Urhan — https://www.linkedin.com/in/kerem-urhan-225870436/ — https://github.com/walsykerem/walsy-3d

use png::{BitDepth, ColorType, Decoder};
use scripts_rs::compose;
use serde_json::Value;
use std::fs;
use std::fs::File;
use std::hint::black_box;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

fn decode_sheet(root: &Path) -> Vec<[u8; 4]> {
    let file = File::open(root.join("examples/apple-sheet.png")).expect("sheet asset");
    let decoder = Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().expect("sheet decode");
    let mut raw = vec![0; reader.output_buffer_size().expect("output size")];
    let info = reader.next_frame(&mut raw).expect("sheet frame");
    assert_eq!((info.width, info.height), (304, 40));
    assert_eq!(info.color_type, ColorType::Rgba);
    assert_eq!(info.bit_depth, BitDepth::Eight);
    raw.truncate(304 * 40 * 4);
    raw.chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect()
}

fn mismatches(a: &[[u8; 4]], b: &[[u8; 4]]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

fn find_chrome() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(custom) = std::env::var_os("APPLE_CHROME") {
        candidates.push(PathBuf::from(custom));
    }
    candidates.extend(
        [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
            "chrome",
        ]
        .iter()
        .map(PathBuf::from),
    );
    candidates.into_iter().find(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

fn run_html(root: &Path, control: bool) -> (Value, String) {
    let chrome = find_chrome().expect("no Chrome/Chromium found; set APPLE_CHROME");
    let source = root.join("tests/global/apple-sheet.html");
    let temp = source.with_file_name(format!(".apple-sheet-control-{}.html", std::process::id()));
    let path = if control {
        let original = fs::read_to_string(&source).expect("harness source");
        let injected = original.replacen(
            "<head>",
            "<head>\n<script>window.__appleMutate = true;</script>",
            1,
        );
        fs::write(&temp, injected).expect("control copy");
        temp.clone()
    } else {
        source
    };
    let url = format!("file://{}", path.display());
    let output = Command::new(chrome)
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
    let tag = "<pre id=\"apple-result\">";
    let start = dom.find(tag).expect("apple-result missing") + tag.len();
    let end = start + dom[start..].find("</pre>").expect("apple-result close");
    let json = dom[start..end].to_string();
    let value: Value = serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("bad apple JSON {json}: {error}"));
    if control {
        let _ = fs::remove_file(&temp);
    }
    (value, json)
}

#[test]
fn global_t_apple_sheet_parity() {
    let root = repo_root();
    let golden = decode_sheet(&root);
    let (width, height, pixels) = compose::sheet("apple").expect("apple sheet");
    assert_eq!((width, height), (304, 40));
    assert_eq!(pixels.len(), golden.len());
    assert_eq!(mismatches(&pixels, &golden), 0, "sheet pixels differ");

    for x0 in [40, 128, 216] {
        for x in x0..x0 + 48 {
            for y in 0..40 {
                assert_eq!(golden[y * width + x][3], 0, "gap not clear at {x},{y}");
            }
        }
    }
    assert!((compose::apple_half_width(30) - 14.54842192175071).abs() < 1e-9);

    let mut body = [[false; 40]; 40];
    for y in 0..40 {
        for x in 0..40 {
            body[y][x] = compose::apple_body(x as i32, y as i32, false);
        }
    }
    let cells = compose::apple_outline_cells(&body);
    assert!(!cells.is_empty());
    assert!(cells.iter().all(|&(x, y)| !body[y][x]));
    let mut flipped = body;
    flipped[0][0] = !flipped[0][0];
    assert_ne!(compose::apple_outline_cells(&flipped), cells, "outline control did not change");

    let view = compose::apple_draw_view("TOP").expect("top view");
    assert_eq!(view.len(), 40 * 40);
    assert!(compose::apple_draw_view("NOPE").is_err());
    let mut damaged = pixels.clone();
    damaged[20 * width + 20][0] ^= 0xff;
    assert!(mismatches(&pixels, &damaged) > 0, "pixel control not detected");
}

#[test]
fn global_t_apple_html_contract() {
    let root = repo_root();
    let html = fs::read_to_string(root.join("tests/global/apple-sheet.html")).expect("harness source");
    for needle in [
        "my linkedin",
        "https://www.linkedin.com/in/kerem-urhan-225870436/",
        "github star me",
        "https://github.com/walsykerem/walsy-3d",
    ] {
        assert!(html.contains(needle), "harness missing: {needle}");
    }
    if find_chrome().is_none() {
        if std::env::var_os("CI").is_some() {
            panic!("no Chrome/Chromium found; set APPLE_CHROME (required in CI)");
        }
        eprintln!("skipping apple HTML contract: no Chrome/Chromium found (set APPLE_CHROME to run)");
        return;
    }
    let (pass, pass_json) = run_html(&root, false);
    println!("PASS JSON: {pass_json}");
    assert_eq!(pass["test"], "apple-sheet-html");
    assert_eq!(pass["control"], false);
    assert_eq!(pass["exit"], 0, "positive run failed: {}", pass["violations"]);
    assert_eq!(pass["marker"], true);
    assert_eq!(pass["transparentGaps"], 3);
    assert_eq!(pass["viewBoxes"], 4);
    assert!(pass["colors"].as_array().expect("colors array").len() <= 8);

    let (fail, fail_json) = run_html(&root, true);
    println!("CONTROL JSON: {fail_json}");
    assert_eq!(fail["control"], true);
    assert_eq!(fail["exit"], 1, "negative control did not fail: {}", fail["violations"]);
    let violations = fail["violations"].as_array().expect("violations array");
    assert!(
        violations.iter().any(|v| v.as_str().unwrap_or("").contains("centre pixel lost")),
        "missing mutation violation: {violations:?}"
    );
}

#[test]
fn global_t_apple_sheet_cost() {
    let started = Instant::now();
    for _ in 0..400 {
        black_box(compose::sheet("apple").expect("apple sheet"));
    }
    let micros = started.elapsed().as_secs_f64() * 1e6 / 400.0;
    println!("RUST_US_PER_SHEET {micros:.1}");
    println!("RUST_SHEETS_PER_SEC {:.1}", 1e6 / micros);
    assert!(micros < 10000.0, "sheet cost out of range: {micros:.1} us");
}
