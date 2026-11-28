use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CHECKS: [&str; 19] = [
    "goal_and_gate",
    "no_setup_first",
    "artifact_chain",
    "keep_scope_exact",
    "creator_editing_boundary",
    "frontmatter_tools",
    "negative_scope_top",
    "per_step_done_when",
    "early_exit_gates",
    "output_contract",
    "high_trigger_description",
    "lean_body",
    "platform_false_positives",
    "cheap_probes_approval",
    "trust_results",
    "no_needless_reruns",
    "cost_visibility",
    "lint_self_audit",
    "delivery_checklist_19",
];

const STEP_HEADINGS: [&str; 5] = [
    "## Step 0. Pick the path",
    "## Step 1. Get the four views",
    "## Step 2. Carve and inspect",
    "## Step 3. Render: loop and numbers",
    "## Step 4. Finish",
];

fn skill_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../SKILL.md")
}

fn read_skill() -> String {
    let path = skill_path();
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn frontmatter(text: &str) -> &str {
    let body = text.strip_prefix("---\n").unwrap_or(text);
    body.split_once("\n---").map_or(body, |(f, _)| f)
}

fn section_bounds(text: &str, heading: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let level = heading.chars().take_while(|c| *c == '#').count();
    let start = lines.iter().position(|line| line.trim() == heading)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| {
            let n = line.chars().take_while(|c| *c == '#').count();
            n > 0 && n <= level && line[n..].starts_with(' ')
        })
        .map_or(lines.len(), |(i, _)| i);
    let begin = lines[..start].iter().map(|l| l.len() + 1).sum();
    let stop: usize = lines[..end].iter().map(|l| l.len() + 1).sum();
    Some((begin, stop.min(text.len())))
}

fn section<'a>(text: &'a str, heading: &str) -> Option<&'a str> {
    let (s, e) = section_bounds(text, heading)?;
    Some(&text[s..e])
}

fn step_block<'a>(text: &'a str, heading: &str) -> Option<&'a str> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|line| line.trim() == heading)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim_start().starts_with("## Step "))
        .map_or(lines.len(), |(i, _)| i);
    let begin: usize = lines[..start].iter().map(|l| l.len() + 1).sum();
    let stop: usize = lines[..end].iter().map(|l| l.len() + 1).sum();
    Some(&text[begin..stop.min(text.len())])
}

fn ordered(text: &str, tokens: &[&str]) -> bool {
    let mut at = 0;
    for token in tokens {
        let Some(i) = text[at..].find(token) else {
            return false;
        };
        at += i + token.len();
    }
    true
}

fn require(errors: &mut Vec<String>, name: &str, ok: bool, detail: impl Into<String>) {
    if !ok {
        errors.push(format!("{name}: {}", detail.into()));
    }
}

fn validate(text: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let front = frontmatter(text);

    let goal = section(text, "## Goal and Gate").unwrap_or_default();
    require(
        &mut errors,
        CHECKS[0],
        goal.contains("Goal:")
            && goal.contains("entry gate")
            && goal.contains("exit gate")
            && goal.contains("No next step before gate"),
        if section(text, "## Goal and Gate").is_none() {
            "section missing: ## Goal and Gate"
        } else {
            "missing Goal / entry gate / exit gate / gate order"
        },
    );

    require(
        &mut errors,
        CHECKS[1],
        text.contains("Start here; do not begin with project scaffolding or tool installation.")
            && goal.contains("`BRIEF.md` written before any other artifact.")
            && !text.to_ascii_lowercase().contains("## setup"),
        "setup-first rule missing",
    );

    let chain = section(text, "### Artifact chain").unwrap_or_default();
    require(
        &mut errors,
        CHECKS[2],
        chain.contains("BRIEF.md → specs.json → PLAN.md → src/outputs/")
            && ordered(
                chain,
                &["BRIEF.md", "specs.json", "PLAN.md", "src/outputs/"],
            ),
        "artifact chain missing or out of order",
    );

    let scope = section(text, "### Keep scope exact").unwrap_or_default();
    require(
        &mut errors,
        CHECKS[3],
        scope.contains("no adjacent features, extra controls, extra engines or extra assets")
            && scope.contains("stop and ask"),
        "keep-scope-exact boundary missing",
    );

    let boundary = section(text, "## Creator editing boundary").unwrap_or_default();
    require(
        &mut errors,
        CHECKS[4],
        boundary.contains("walsy:user-begin")
            && boundary.contains("walsy:user-end")
            && boundary.contains("Generated artifacts belong in `src/outputs/`"),
        "creator editing boundary missing marker or output root",
    );

    let allowed = front
        .lines()
        .find_map(|line| line.trim().strip_prefix("allowed-tools:"))
        .unwrap_or_default();
    require(
        &mut errors,
        CHECKS[5],
        front.contains("disable-model-invocation: true")
            && ["Read", "Write", "Edit", "Bash", "Glob", "Grep"]
                .iter()
                .all(|t| allowed.split(',').any(|v| v.trim() == *t))
            && ["walsy_carve", "walsy_lint", "walsy_preview", "walsy_viewer"]
                .iter()
                .all(|t| allowed.split(',').any(|v| v.trim() == *t)),
        "frontmatter tool surface missing",
    );

    let top: String = text.lines().take(12).collect::<Vec<_>>().join("\n");
    require(
        &mut errors,
        CHECKS[6],
        top.lines().any(|line| line.trim() == "## Negative scope")
            && matches!(
                (text.find("## Negative scope"), text.find("## Goal and Gate")),
                (Some(a), Some(b)) if a < b
            ),
        "negative scope not at top",
    );

    let done = text
        .lines()
        .filter(|line| line.trim() == "## Done when")
        .count();
    require(
        &mut errors,
        CHECKS[7],
        done >= 5
            && STEP_HEADINGS.iter().all(|heading| {
                step_block(text, heading).is_some_and(|body| {
                    body.contains("## Done when")
                        && body.contains(&format!(
                            "**Step {}:**",
                            heading.chars().nth(8).unwrap_or('?')
                        ))
                })
            }),
        format!("per-step done-when count {done}"),
    );

    let early = section(text, "### Early exit and discount gates").unwrap_or_default();
    require(
        &mut errors,
        CHECKS[8],
        ["Nothing turns", "Supplied sheet turns correctly", "skip image synthesis", "Report every discount"]
            .iter()
            .all(|t| early.contains(t)),
        "early-exit / discount gate missing",
    );

    require(
        &mut errors,
        CHECKS[9],
        chain.contains("Programmatic output contracts are tied to the `check` / `verify` test scripts")
            && chain.contains("missing key, wrong dimension or missing file; no exceptions"),
        "output contract not tied to check / verify",
    );

    let description = front
        .lines()
        .find_map(|line| line.trim().strip_prefix("description:"))
        .unwrap_or_default();
    require(
        &mut errors,
        CHECKS[10],
        ordered(
            description,
            &[
                "walsy-3d",
                "fake 3D pixel art",
                "pixel art turntable",
                "stacked slices",
                "turn a top-down sprite without drawing every angle",
            ],
        ),
        "description trigger phrases missing",
    );

    require(
        &mut errors,
        CHECKS[11],
        text.lines().count() <= 260
            && text.contains("Long recipes stay in `references/engines.md`; do not copy them here."),
        format!("body lines {}", text.lines().count()),
    );

    let traps = section(text, "### Traps and false positives").unwrap_or_default();
    require(
        &mut errors,
        CHECKS[12],
        ["HiDPI backing size", "Godot viewport stretch", "Unity Pixels Per Unit", "flat white vs transparent background", "JPEG fringe"]
            .iter()
            .all(|t| traps.contains(t)),
        "platform / false-positive checks missing",
    );

    let probes = section(text, "### Cheap probes").unwrap_or_default();
    require(
        &mut errors,
        CHECKS[13],
        ordered(
            probes,
            &["walsy_lint", "check.png", "diff", "Render at delivery size"],
        ) && text.contains("**Approval gate:**")
            && text.contains("image generation, dense re-carve, batch exports, installs and writes outside"),
        "cheap probes or approval gate missing",
    );

    require(
        &mut errors,
        CHECKS[14],
        text.contains("**Trust results, not logs:** trust result artifacts, not log text."),
        "trust-results-not-logs rule missing",
    );

    require(
        &mut errors,
        CHECKS[15],
        text.contains("Never re-run a passing gate")
            && text.contains("re-run only if the artifact changed"),
        "needless rerun rule missing",
    );

    require(
        &mut errors,
        CHECKS[16],
        text.contains("**Cost visibility:** report per-step cost and what was skipped. Cheap probes first; expensive work last."),
        "cost visibility rule missing",
    );

    require(
        &mut errors,
        CHECKS[17],
        text.contains("cargo run --offline --quiet --manifest-path rust/lint-skill/Cargo.toml -- check")
            && text.contains("cargo run --offline --quiet --manifest-path rust/lint-skill/Cargo.toml -- verify"),
        "self-auditing commands missing",
    );

    let exit_gate = section(text, "## Goal and Gate").unwrap_or_default();
    let unique: HashSet<&str> = CHECKS.iter().copied().collect();
    require(
        &mut errors,
        CHECKS[18],
        CHECKS.len() == 19
            && unique.len() == CHECKS.len()
            && exit_gate.contains("Delivery-size visual check passed")
            && exit_gate.contains("`check.png` and the full turn reviewed")
            && exit_gate.contains("`check` and `verify` both pass")
            && exit_gate.contains("Per-step cost and skipped work reported"),
        "delivery checklist not complete",
    );

    errors
}

#[test]
fn real_skill_passes_all_nineteen_checks() {
    let text = read_skill();
    let errors = validate(&text);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert_eq!(CHECKS.len(), 19);
}

#[test]
fn negative_control_missing_goal_gate_section_fails_by_name() {
    let text = read_skill();
    let edited = section_bounds(&text, "## Goal and Gate")
        .map(|(s, e)| format!("{}{}", &text[..s], &text[e..]))
        .expect("fixture source must contain ## Goal and Gate");

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let temp = std::env::temp_dir().join(format!(
        "walsy-gate-chain-{}-{nonce}.md",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .unwrap_or_else(|e| panic!("create {}: {e}", temp.display()));
    file.write_all(edited.as_bytes())
        .unwrap_or_else(|e| panic!("write {}: {e}", temp.display()));
    drop(file);

    let reloaded = fs::read_to_string(&temp).unwrap_or_else(|e| panic!("read {}: {e}", temp.display()));
    let errors = validate(&reloaded);
    let _ = fs::remove_file(&temp);

    assert!(!errors.is_empty(), "negative control unexpectedly passed");
    let expected = "goal_and_gate: section missing: ## Goal and Gate";
    assert!(
        errors.iter().any(|e| e == expected),
        "expected exact failure {expected:?}, got:\n{}",
        errors.join("\n")
    );
}
