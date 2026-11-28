use std::path::{Path, PathBuf};

const LINKEDIN_URL: &str = "https://www.linkedin.com/in/kerem-urhan-225870436/";
const GITHUB_URL: &str = "https://github.com/walsykerem/walsy-3d";
const MARKER: &str = "/*__EMBEDDED_STACK__*/null";

const HTML_FILES: [&str; 3] = [
    "examples/dancing-dog/dancing-dog.html",
    "examples/dancing-dog/viewer.html",
    "rust/scripts-rs/src/viewer.html",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn has_pair(html: &str, url: &str, label: &str) -> bool {
    html.contains(&format!("href=\"{url}\"")) && html.contains(&format!(">{label}</a>"))
}

#[test]
fn every_shipped_html_has_both_exact_links() {
    for rel in HTML_FILES {
        let html = read(rel);
        assert!(
            has_pair(&html, LINKEDIN_URL, "my linkedin"),
            "{rel}: linkedin pair missing"
        );
        assert!(
            has_pair(&html, GITHUB_URL, "github star me"),
            "{rel}: github pair missing"
        );
        assert_eq!(html.matches(LINKEDIN_URL).count(), 1, "{rel}: linkedin URL count");
        assert_eq!(html.matches(GITHUB_URL).count(), 1, "{rel}: github URL count");
    }
}

#[test]
fn generated_viewer_keeps_payload_and_template_keeps_marker() {
    let template = read("rust/scripts-rs/src/viewer.html");
    assert_eq!(template.matches(MARKER).count(), 1, "template marker");

    let viewer = read("examples/dancing-dog/viewer.html");
    for key in [
        "title",
        "slices",
        "sliceWidth",
        "sliceHeight",
        "sheet",
        "top",
        "side",
        "front",
        "back",
    ] {
        assert!(
            viewer.contains(&format!("\"{key}\":")),
            "viewer payload missing {key}"
        );
    }
    assert!(!viewer.contains(MARKER), "viewer payload not embedded");
}

#[test]
fn negative_control_rejects_missing_or_wrong_pair() {
    let clean = "<header><div id=\"meta\">loading…</div></header>";
    assert!(!has_pair(clean, LINKEDIN_URL, "my linkedin"));
    assert!(!has_pair(clean, GITHUB_URL, "github star me"));

    let wrong_label = format!("<a href=\"{LINKEDIN_URL}\">wrong label</a>");
    assert!(!has_pair(&wrong_label, LINKEDIN_URL, "my linkedin"));

    let wrong_url = "<a href=\"https://example.invalid/\">my linkedin</a>";
    assert!(!has_pair(wrong_url, LINKEDIN_URL, "my linkedin"));
}
