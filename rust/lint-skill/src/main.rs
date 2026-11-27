use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

const DEFAULT_SKILL_MD: &str = "../../SKILL.md";

const TOOL_SURFACE: &[&[&str]] = &[
    &["walsy", "_carve"],
    &["walsy", "_preview"],
    &["walsy", "_viewer"],
    &["walsy", "_lint"],
];

const PAYLOAD_KEYS: &[&str] = &[
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

const REQUIRED_TOKENS: &[(&str, &str)] = &[
    ("negative scope near top", "negative scope"),
    ("Goal", "## Goal"),
    ("Gate", "Gate"),
    ("artifact BRIEF.md", "BRIEF.md"),
    ("artifact specs.json", "specs.json"),
    ("artifact PLAN.md", "PLAN.md"),
    ("artifact src/outputs/", "src/outputs/"),
    ("Keep scope exact", "Keep scope exact"),
    ("Creator editing boundary", "## Creator editing boundary"),
    ("done when", "done when"),
    ("early exit", "early exit"),
    ("discount gate", "discount gate"),
    ("cheap probe", "cheap probe"),
    ("Trust results, not logs", "Trust results, not logs"),
    ("no re-runs", "re-run a passing gate"),
    ("cost visibility", "cost visibility"),
    ("platform trap", "HiDPI"),
    ("false positive", "false positive"),
    ("lint Cargo.toml", "rust/lint-skill/Cargo.toml"),
    ("lint source", "rust/lint-skill/src/main.rs"),
    ("Rust + HTML only", "Rust + HTML only"),
    ("walsy flag prefix", "--walsy-"),
    ("rotation pivot", "(w - 1) * 0.5"),
    ("Math.round", "Math.round"),
    ("imageSmoothingEnabled", "imageSmoothingEnabled"),
];

const DELIVERY_CHECKLIST: &[(&str, &str, &str)] = &[
    ("1", "Goal + Gate", "## Goal and Gate"),
    ("2", "No setup-first start", "do not begin with project scaffolding"),
    ("3", "Fixed artifact chain + filename contract", "### Artifact chain"),
    ("4", "Clean format/resolution/file-remnant structure", "remove format/resolution/file residue"),
    ("5", "Keep scope exact for borrowed workflows", "### Keep scope exact"),
    ("6", "Creator editing boundary", "## Creator editing boundary"),
    ("7", "Pre-approved frontmatter, tool-surface limit", "disable-model-invocation: true"),
    ("8", "Negative scope and boundaries at top", "## Negative scope"),
    ("9", "Observable done-when per step", "## Done when"),
    ("10", "Early-exit and discount gates", "### Early exit and discount gates"),
    ("11", "Programmatic output contracts tied to test/verify", "Programmatic output contracts"),
    ("12", "One-job, high-trigger description", "Build fake-3D rotatable pixel art"),
    ("13", "Lean body, flexible exploration, no ceremony", "add no ceremonial scaffolding"),
    ("14", "Platform traps + false positives", "### Traps and false positives"),
    ("15", "Cheap probes first; approval before expensive work", "### Cheap probes"),
    ("16", "Trust results, not logs", "Trust results, not logs"),
    ("17", "Avoid needless re-runs", "re-run a passing gate"),
    ("18", "Cost and spend visibility", "Cost visibility"),
    ("19", "Self-auditing lint script inside the skill", "cargo run --offline --quiet --manifest-path rust/lint-skill/Cargo.toml -- check"),
];

const REQUIRED_HTML_LINKS: &[(&str, &str)] = &[
    ("my linkedin", "https://www.linkedin.com/in/kerem-urhan-225870436/"),
    ("github star me", "https://github.com/walsykerem/walsy-3d"),
];

const GLOBAL_TEST_RULE: &str = "Test dosyalarından mutlaka test et: `tests/global/*.html` ve `rust/*/tests/global_t*.rs` dosyaları `check`/`verify` öncesinde çalıştırılır; negatif kontrol sonucu raporlanır.";

const FRAGMENTS: &[&[&str]] = &[
    &["li", "cense"],
    &["lic", "ence"],
    &["copy", "right"],
    &["attri", "bution"],
    &["deri", "ved from"],
    &["pyt", "hon"],
    &["rust", "-anything"],
    &["try", "cmd"],
    &["re", "play"],
    &["Neon", " Street"],
    &["Meadow", " Island"],
    &["Add", " Prop"],
    &["drag", " to orbit"],
    &["drag", "to orbit"],
    &["Drop", " your PNG"],
    &["drop", " PNGs"],
    &["old", " stack"],
    &["walsy", "_wor",
      "ld"],
];

fn assembled(parts: &[&str]) -> String {
    parts.concat()
}

fn word_hit(hay: &str, needle: &str) -> bool {
    // Match on a word-char-only stream so fragments split by spaces, quotes or
    // newlines still assemble; boundaries are checked in the original text.
    let mut stream = String::new();
    let mut positions = Vec::new();
    for (i, c) in hay.char_indices() {
        if c.is_ascii_alphanumeric() || c == '_' {
            stream.push(c.to_ascii_lowercase());
            positions.push(i);
        }
    }
    let n: String = needle
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .flat_map(char::to_lowercase)
        .collect();
    if n.len() < 4 {
        return false;
    }
    let boundary = |c: Option<char>| c.map_or(true, |c| !c.is_ascii_alphanumeric() && c != '_');
    let mut from = 0;
    while let Some(i) = stream[from..].find(&n) {
        let s = from + i;
        let e = s + n.len();
        let left = hay[..positions[s]].chars().next_back();
        let end = positions[e - 1] + 1;
        let right = hay[end..].chars().next();
        if boundary(left) && boundary(right) {
            return true;
        }
        from = if e > s { e } else { s + 1 };
    }
    false
}

fn banned_patterns() -> Vec<(String, String)> {
    FRAGMENTS
        .iter()
        .map(|f| (assembled(f), f.join("|")))
        .collect()
}

fn section_bounds(text: &str, heading: &str) -> Option<(usize, usize)> {
    let level = heading.chars().take_while(|c| *c == '#').count();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.trim() == heading)?;
    let prefix: usize = lines[..start].iter().map(|l| l.len() + 1).sum();
    let end = lines[start + 1..]
        .iter()
        .position(|l| {
            let n = l.chars().take_while(|c| *c == '#').count();
            n > 0 && n <= level && l[n..].starts_with(' ')
        })
        .map_or(text.len(), |i| {
            lines[..start + 1 + i].iter().map(|l| l.len() + 1).sum()
        });
    Some((prefix, end))
}

fn artifact_chain_errors(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let sec = section_bounds(text, "### Artifact chain")
        .or_else(|| section_bounds(text, "## Fixed artifact chain"));
    let Some((s, e)) = sec else {
        return vec!["section missing: ### Artifact chain".to_string()];
    };
    let sec = text[s..e].to_ascii_lowercase();
    let mut prev = None;
    for token in ["BRIEF.md", "specs.json", "PLAN.md", "src/outputs/"] {
        let pos = sec.find(&token.to_ascii_lowercase());
        match (pos, prev) {
            (Some(i), Some(p)) if i <= p => {
                out.push(format!("Artifact chain out of order: {token}"));
                break;
            }
            (Some(i), _) => prev = Some(i),
            (None, _) => {
                out.push(format!("Artifact chain missing: {token}"));
                break;
            }
        }
    }
    out
}

fn delivery_checklist_errors(text: &str) -> Vec<String> {
    let Some((s, e)) = section_bounds(text, "## Delivery checklist") else {
        return vec!["section missing: ## Delivery checklist".to_string()];
    };
    let sec = &text[s..e];
    let mut out = Vec::new();
    for (num, label, target) in DELIVERY_CHECKLIST {
        if !sec.lines().any(|line| line.starts_with(&format!("| {num} |"))) {
            out.push(format!("Delivery checklist row {num} missing ({label})"));
            continue;
        }
        if !sec.contains(label) {
            out.push(format!("Delivery checklist row {num} label missing: {label}"));
        }
        if !text.contains(target) {
            out.push(format!("Delivery checklist row {num} target missing: {target}"));
        }
    }
    let rows = sec
        .lines()
        .filter(|line| {
            line.starts_with("| ")
                && line[2..].chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .count();
    if rows != DELIVERY_CHECKLIST.len() {
        out.push(format!("Delivery checklist rows {rows} != {}", DELIVERY_CHECKLIST.len()));
    }
    out
}

fn has_html_link(html: &str, label: &str, url: &str) -> bool {
    let needle = format!("href=\"{url}\"");
    let mut from = 0;
    while let Some(i) = html.get(from..).and_then(|s| s.find(&needle)) {
        let start = from + i;
        let end = html[start..]
            .find("</a>")
            .map(|j| start + j + 4)
            .unwrap_or(html.len());
        if html[start..end].contains(&format!(">{label}<")) {
            return true;
        }
        from = start + needle.len();
        if from >= html.len() {
            break;
        }
    }
    false
}

fn missing_html_links(html: &str) -> Vec<&'static str> {
    REQUIRED_HTML_LINKS
        .iter()
        .filter(|(label, url)| !has_html_link(html, label, url))
        .map(|(_, url)| *url)
        .collect()
}

fn html_link_errors(root: &Path, files: &[PathBuf]) -> Vec<String> {
    let mut out = Vec::new();
    for path in files
        .iter()
        .filter(|p| p.extension() == Some(OsStr::new("html")))
    {
        let Ok(html) = fs::read_to_string(path) else {
            out.push(format!("html unreadable: {}", path.display()));
            continue;
        };
        let rel = path.strip_prefix(root).unwrap_or(path);
        for url in missing_html_links(&html) {
            out.push(format!("html link missing in {}: {}", rel.display(), url));
        }
    }
    out
}

fn global_test_rule_errors(text: &str) -> Vec<String> {
    if text.contains(GLOBAL_TEST_RULE) {
        vec![]
    } else {
        vec![format!("global test rule sentence missing: {GLOBAL_TEST_RULE}")]
    }
}

fn structural_errors(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = text.to_ascii_lowercase();
    let top_lines: Vec<&str> = text.lines().take(60).collect();
    let top: String = top_lines.join("\n").to_ascii_lowercase();

    for heading in ["## Negative scope", "## Goal and Gate"] {
        if !top.lines().any(|l| l.trim().eq_ignore_ascii_case(heading)) {
            out.push(format!("section missing in first 60 lines: {heading}"));
        }
    }
    if let Some((s, e)) = section_bounds(text, "## Goal and Gate") {
        for tok in ["entry gate", "exit gate"] {
            if !lower[s..e].contains(tok) {
                out.push(format!("Goal and Gate missing: {tok}"));
            }
        }
    }

    out.extend(artifact_chain_errors(text));

    if let Some((s, e)) = section_bounds(text, "## Creator editing boundary") {
        let sec = &text[s..e];
        for marker in ["walsy:user-begin", "walsy:user-end"] {
            if !sec.contains(marker) {
                out.push(format!("Creator editing boundary missing: {marker}"));
            }
        }
    } else {
        out.push("section missing: ## Creator editing boundary".to_string());
    }

    let count = lower.match_indices("done when").count();
    if count < 4 {
        out.push(format!("done when count {count} < 4"));
    }
    for tok in [
        "Keep scope exact",
        "cost visibility",
        "trust results",
    ] {
        if !lower.contains(&tok.to_ascii_lowercase()) {
            out.push(format!("missing token: {tok}"));
        }
    }
    if !lower.contains("early exit") && !lower.contains("discount gate") {
        out.push("missing token: early exit or discount gate".to_string());
    }
    out
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("check");
    let code = match mode {
        "check" => check(args.get(2).map(String::as_str)),
        "verify" => verify(&args[2..]),
        other => {
            eprintln!("usage: lint-skill [check [SKILL.md]] | verify [--changed-only]");
            eprintln!("unknown mode: {other}");
            1
        }
    };
    std::process::exit(code);
}

fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn skill_dir() -> PathBuf {
    crate_dir().parent().and_then(Path::parent).unwrap().to_path_buf()
}

fn check(override_md: Option<&str>) -> i32 {
    let root = skill_dir();
    let skill_md = match override_md {
        Some(p) => PathBuf::from(p),
        None => crate_dir().join(DEFAULT_SKILL_MD),
    };
    let mut missing = Vec::new();

    let text = match fs::read_to_string(&skill_md) {
        Ok(t) => t,
        Err(e) => {
            println!("CHECK FAIL");
            println!("- {} ({}): {}", skill_md.display(), e, skill_md.display());
            return 1;
        }
    };

    let front = frontmatter(&text).unwrap_or_default();
    let f = front.to_ascii_lowercase();
    if !front.lines().any(|l| {
        let mut it = l.splitn(2, ':');
        it.next().map(str::trim) == Some("name")
            && it.next().map(str::trim) == Some("walsy-3d")
    }) {
        missing.push("frontmatter: name: walsy-3d".to_string());
    }
    if !front.lines().any(|l| {
        let mut it = l.splitn(2, ':');
        it.next().map(str::trim) == Some("description")
            && it.next().is_some_and(|v| !v.trim().is_empty())
    }) {
        missing.push("frontmatter: non-empty description naming the single main job".to_string());
    }
    let allowed = f.lines().find_map(|l| {
        l.trim_start()
            .strip_prefix("allowed-tools:")
            .map(|v| v.split(',').map(str::trim).collect::<Vec<_>>())
    });
    let allowed = allowed.unwrap_or_else(|| {
        missing.push("frontmatter: allowed-tools".to_string());
        Vec::new()
    });
    if !f.contains("disable-model-invocation: true") {
        missing.push("frontmatter: disable-model-invocation: true".to_string());
    }
    for parts in TOOL_SURFACE {
        let token = assembled(parts);
        if !allowed.iter().any(|t| t.eq_ignore_ascii_case(&token)) {
            missing.push(format!("frontmatter: tool surface {token}"));
        }
    }

    let body_lower = text.to_ascii_lowercase();
    let top: String = text.lines().take(20).collect::<Vec<_>>().join("\n");
    let top_lower = top.to_ascii_lowercase();
    if !top_lower.contains("limit")
        && !top_lower.contains("not supported")
        && !top_lower.contains("negative scope")
    {
        missing.push("negative scope / limits near top".to_string());
    }
    if let Some(pos) = top_lower.find("## setup") {
        if body_lower[pos..].starts_with("## setup") {
            missing.push("no Setup-first rule (Setup appears before Gate)".to_string());
        }
    }
    for (label, tok) in REQUIRED_TOKENS {
        let needle = tok.to_ascii_lowercase();
        if !body_lower.contains(&needle) {
            missing.push(format!("token: {label} ({tok})"));
        }
    }
    missing.extend(structural_errors(&text));
    missing.extend(delivery_checklist_errors(&text));
    missing.extend(global_test_rule_errors(&text));

    for (pat, desc) in banned_patterns() {
        if pat.len() < 4 {
            continue;
        }
        if word_hit(&text, &pat) {
            missing.push(format!("banned token in {}: {}", skill_md.display(), desc));
        }
    }

    let mut files = Vec::new();
    walk(&root, &mut files, &mut missing);
    if files.iter().any(|p| p.extension() == Some(OsStr::new("py"))) {
        missing.push("forbidden .py file in skill tree".to_string());
    }
    missing.extend(html_link_errors(&root, &files));
    for path in &files {
        // The linter's own source lists every banned fragment as data.
        if path.file_name() == Some(OsStr::new("SKILL.md"))
            || path.starts_with(&crate_dir())
        {
            continue;
        }
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "png" || ext == "lock" || rel.to_string_lossy().starts_with("target/") {
            continue;
        }
        let Ok(content) = fs::read_to_string(path) else { continue };
        for (pat, desc) in banned_patterns() {
            if pat.len() < 4 {
                continue;
            }
            if word_hit(&content, &pat) {
                missing.push(format!("banned token in {}: {}", rel.display(), desc));
            }
        }
    }

    missing.sort();
    missing.dedup();
    if missing.is_empty() {
        println!("CHECK PASS");
        0
    } else {
        println!("CHECK FAIL");
        for m in missing {
            println!("- {m}");
        }
        1
    }
}

fn frontmatter(text: &str) -> Option<&str> {
    let trimmed = text.strip_prefix("---\n")?;
    let end = trimmed.find("\n---")?;
    Some(&trimmed[..end])
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, bad: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for ent in rd.flatten() {
        let p = ent.path();
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        if name == "target" || name == ".git" {
            continue;
        }
        if ent.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
            bad.push(format!("symlink in skill tree: {}", p.display()));
            continue;
        }
        if p.is_dir() {
            walk(&p, out, bad);
        } else {
            out.push(p);
        }
    }
}

fn verify(args: &[String]) -> i32 {
    let root = skill_dir();
    let changed_only = args.iter().any(|a| a == "--changed-only");
    let app = root.join("rust/app");
    let manifest = app.join("Cargo.toml");
    let target = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("walsy-lint-target"));
    let binary = target.join("release/views_to_slices");

    if changed_only && binary.exists() {
        let Ok(bin_mtime) = fs::metadata(&binary).and_then(|m| m.modified()) else {
            // fall through to build
            return run_verify(&root, &manifest, &binary, &target);
        };
        let mut sources = Vec::new();
        newest_in(&app.join("src"), &mut sources);
        newest_in(&app.join("."), &mut sources);
        newest_in(&root.join("rust/scripts-rs/src"), &mut sources);
        sources.push(root.join("rust/scripts-rs/src/viewer.html"));
        sources.push(root.join("SKILL.md"));
        sources.push(root.join("examples/dancing-dog/sheet.png"));
        let newest = sources
            .iter()
            .filter_map(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
            .max()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        if newest <= bin_mtime {
            println!("VERIFY SKIP unchanged");
            return 0;
        }
    }
    run_verify(&root, &manifest, &binary, &target)
}

fn newest_in(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            newest_in(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn run_verify(root: &Path, manifest: &Path, binary: &Path, target: &Path) -> i32 {
    let build = Command::new("cargo")
        .args(["build", "--offline", "--release", "--manifest-path"])
        .arg(manifest)
        .env("CARGO_TARGET_DIR", target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match build {
        Ok(s) if s.success() => println!("VERIFY build OK"),
        Ok(s) => {
            println!("VERIFY FAIL build exit {}", s.code().unwrap_or(-1));
            return 1;
        }
        Err(e) => {
            println!("VERIFY FAIL build: {e}");
            return 1;
        }
    }
    if !binary.exists() {
        println!("VERIFY FAIL missing binary: {}", binary.display());
        return 1;
    }

    let tmp = env::temp_dir().join(format!("walsy-lint-verify-{}", std::process::id()));
    if let Err(e) = fs::create_dir_all(&tmp) {
        println!("VERIFY FAIL temp dir: {e}");
        return 1;
    }
    let preview = tmp.join("check.png");
    let viewer = tmp.join("viewer.html");
    let out = tmp.join("slices.png");
    let sheet = root.join("examples/dancing-dog/sheet.png");

    let carved = Command::new(binary)
        .arg(&sheet)
        .arg(&out)
        .args([
            "--walsy-order", "reading",
            "--walsy-top-nose", "down",
            "--walsy-side-nose", "right",
            "--walsy-preview",
        ])
        .arg(&preview)
        .args(["--walsy-html"])
        .arg(&viewer)
        .output();
    let carved = match carved {
        Ok(o) => o,
        Err(e) => {
            println!("VERIFY FAIL carve: {e}");
            return 1;
        }
    };
    let stdout = String::from_utf8_lossy(&carved.stdout);
    if !carved.status.success() {
        println!("VERIFY FAIL carve exit {}", carved.status.code().unwrap_or(-1));
        return 1;
    }
    if !stdout.contains("36 slices of 30 x 65 px") {
        println!("VERIFY FAIL carve stdout missing slice geometry");
        return 1;
    }
    println!("VERIFY carve OK: 36 slices of 30 x 65 px");

    if !nonempty(&preview) || !nonempty(&viewer) {
        println!("VERIFY FAIL preview or viewer missing/empty");
        return 1;
    }
    println!("VERIFY preview + viewer OK");
    let html = fs::read_to_string(&viewer).unwrap_or_default();
    if !html.contains("const EMBEDDED") {
        println!("VERIFY FAIL viewer missing const EMBEDDED");
        return 1;
    }
    let missing: Vec<_> = PAYLOAD_KEYS
        .iter()
        .filter(|k| !html.contains(&format!("\"{k}\"")))
        .copied()
        .collect();
    if !missing.is_empty() {
        println!("VERIFY FAIL viewer payload keys: {}", missing.join(", "));
        return 1;
    }
    println!("VERIFY viewer payload OK (9 keys)");

    let chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    if !Path::new(chrome).exists() {
        println!("SKIP visual");
        let _ = fs::remove_dir_all(&tmp);
        return 0;
    }
    // Pin two different angles instead of relying on virtual-time rAF cadence;
    // headless Chrome can stop advancing rAF before the screenshot deadline.
    let frames = ["a.png", "b.png"].map(|n| tmp.join(n));
    for (frame, angle) in frames.iter().zip(["70", "250"]) {
        let injected = format!("<script>state.playing=false;let e=document.getElementById('angle');e.value='{angle}';e.dispatchEvent(new Event('input',{{bubbles:true}}));</script>");
        let animated = tmp.join(format!("animated-{angle}.html"));
        if fs::write(&animated, html.replace("</body>", &format!("{injected}</body>")).as_bytes()).is_err() {
            println!("VERIFY FAIL write animated viewer");
            return 1;
        }
        let url = format!("file://{}", animated.display());
        let outarg = format!("--screenshot={}", frame.display());
        let virt = "--virtual-time-budget=2500";
        let res = Command::new(chrome)
            .args([
                "--headless", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                "--window-size=900,700", &virt, &outarg, &url,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match res {
            Ok(s) if s.success() => {},
            Ok(s) => {
                println!("VERIFY FAIL chrome exit {}", s.code().unwrap_or(-1));
                return 1;
            }
            Err(e) => {
                println!("VERIFY FAIL chrome: {e}");
                return 1;
            }
        }
    }
    if !nonempty(&frames[0]) || !nonempty(&frames[1]) {
        println!("VERIFY FAIL chrome produced empty frames");
        return 1;
    }
    let a = fs::read(&frames[0]).unwrap_or_default();
    let b = fs::read(&frames[1]).unwrap_or_default();
    if a == b {
        println!("VERIFY FAIL visual frames identical");
        return 1;
    }
    println!("VERIFY visual OK: frames differ");
    let _ = fs::remove_dir_all(&tmp);
    0
}

fn nonempty(p: &Path) -> bool {
    fs::metadata(p).map(|m| m.is_file() && m.len() > 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "---\nname: walsy-3d\ndescription: test\ndisable-model-invocation: true\n---\n\
## Negative scope\nlimits here\n\n\
## Goal and Gate\nentry gate then exit gate\n\n\
### Artifact chain\nBRIEF.md then specs.json then PLAN.md then src/outputs/\n\n\
### Keep scope exact\nKeep scope exact\n\n\
## Creator editing boundary\nbetween walsy:user-begin and walsy:user-end\n\n\
DonE when 1\nDone when 2\nDONE WHEN 3\ndone when 4\n\
early exit; discount gate; cost visibility; trust results\n";

    #[test]
    fn fixture_passes() {
        assert!(structural_errors(FIXTURE).is_empty());
    }

    #[test]
    fn missing_goal_or_negative_scope_fails() {
        let text = FIXTURE.replace("## Goal and Gate\n", "");
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "section missing in first 60 lines: ## Goal and Gate"));
        let text = FIXTURE.replace("## Negative scope\n", "");
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "section missing in first 60 lines: ## Negative scope"));
    }

    #[test]
    fn goal_gate_terms_required() {
        let text = FIXTURE.replace("entry gate then exit gate", "proceed directly");
        let errs = structural_errors(&text);
        assert!(errs.iter().any(|e| e == "Goal and Gate missing: entry gate"));
        assert!(errs.iter().any(|e| e == "Goal and Gate missing: exit gate"));
    }

    #[test]
    fn artifact_chain_order_checked() {
        let text = FIXTURE.replace(
            "BRIEF.md then specs.json then PLAN.md then src/outputs/",
            "BRIEF.md then PLAN.md then specs.json then src/outputs/",
        );
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "Artifact chain out of order: PLAN.md"));
        let text = FIXTURE.replace("specs.json then ", "");
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "Artifact chain missing: specs.json"));
    }

    #[test]
    fn creator_markers_required() {
        let text = FIXTURE.replace(" walsy:user-end", "");
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "Creator editing boundary missing: walsy:user-end"));
    }

    #[test]
    fn count_and_tokens_checked() {
        let text = FIXTURE.replace("done when 4\n", "");
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "done when count 3 < 4"));
        let text = FIXTURE.replace("cost visibility; ", "");
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "missing token: cost visibility"));
        let text = FIXTURE.replace("early exit; discount gate; ", "");
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "missing token: early exit or discount gate"));
        let text = FIXTURE.replace("trust results", "trust nothing");
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "missing token: trust results"));
        let text = FIXTURE.replace("Keep scope exact\n", "");
        assert!(structural_errors(&text)
            .iter()
            .any(|e| e == "missing token: Keep scope exact"));
    }

    #[test]
    fn delivery_checklist_requires_all_19_rows() {
        let mut text = String::from("## Delivery checklist\n\n");
        for (num, label, target) in DELIVERY_CHECKLIST {
            text.push_str(&format!("| {num} | {label} | `{target}` |\n"));
        }
        assert!(delivery_checklist_errors(&text).is_empty());
        let text = text.replacen("| 19 |", "| 20 |", 1);
        assert!(delivery_checklist_errors(&text)
            .iter()
            .any(|e| e.contains("row 19")));
    }

    #[test]
    fn html_missing_link_fails() {
        let tmp = std::env::temp_dir().join(format!(
            "walsy-lint-missing-link-{}.html",
            std::process::id()
        ));
        let html = "<a href=\"https://www.linkedin.com/in/kerem-urhan-225870436/\">my linkedin</a>";
        fs::write(&tmp, html).expect("write temp html");
        let content = fs::read_to_string(&tmp).expect("read temp html");
        let missing = missing_html_links(&content);
        assert!(
            missing.contains(&"https://github.com/walsykerem/walsy-3d"),
            "{missing:?}"
        );
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn global_test_rule_is_required() {
        let text = format!("## Delivery checklist\n\n{GLOBAL_TEST_RULE}\n");
        assert!(global_test_rule_errors(&text).is_empty());
        let text = text.replace(GLOBAL_TEST_RULE, "");
        assert!(!global_test_rule_errors(&text).is_empty());
    }
}
