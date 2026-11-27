use std::fs;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

// Full valid skill: every token the real `lint-skill check` requires.
const VALID: &str = "\
---
name: walsy-3d
description: Build rotatable sprite-stack models from PNG slices.
allowed-tools: walsy_carve, walsy_lint, walsy_preview, walsy_viewer
disable-model-invocation: true
---
## Negative scope
limits: no arbitrary 3D scenes; no unrelated tooling. road. replayable road.

## Goal and Gate
entry gate then exit gate.

### Artifact chain
BRIEF.md then specs.json then PLAN.md then src/outputs/

### Keep scope exact
Keep scope exact.

## Creator editing boundary
between walsy:user-begin and walsy:user-end.

done when 1
Done when 2
DONE WHEN 3
done when 4
early exit; discount gate; cheap probe; cost visibility; Trust results, not logs; no re-runs: do not re-run a passing gate; platform trap: HiDPI; false positive; lint Cargo.toml: rust/lint-skill/Cargo.toml; rotation pivot: (w - 1) * 0.5; Math.round; imageSmoothingEnabled.
";

const ALL_TOOLS: [&str; 4] = ["walsy_carve", "walsy_lint", "walsy_preview", "walsy_viewer"];
static SEQ: AtomicU64 = AtomicU64::new(0);

fn with_line(text: &str, prefix: &str, line: &str) -> String {
    let mut out = text
        .lines()
        .map(|l| if l.starts_with(prefix) { line } else { l })
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}

fn run(text: &str) -> Output {
    let dir = std::env::temp_dir().join(format!(
        "walsy-lint-t09-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    let skill = dir.join("SKILL.md");
    fs::write(&skill, text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_lint-skill"))
        .arg("check")
        .arg(&skill)
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(dir);
    out
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

#[test]
fn valid_skill_passes_real_check() {
    let out = run(VALID);
    assert!(out.status.success(), "{}", stdout(&out));
    assert_eq!(stdout(&out), "CHECK PASS\n");
}

#[test]
fn each_missing_required_tool_fails_with_exact_error() {
    for dropped in ALL_TOOLS {
        let kept: Vec<&str> = ALL_TOOLS.iter().copied().filter(|t| *t != dropped).collect();
        let line = format!("allowed-tools: {}", kept.join(", "));
        let out = run(&with_line(
            VALID,
            "allowed-tools:",
            &line,
        ));
        assert!(!out.status.success(), "missing {dropped} passed");
        let expected = format!("- frontmatter: tool surface {dropped}");
        assert!(
            stdout(&out).lines().any(|l| l == expected),
            "missing {dropped}: {}",
            stdout(&out)
        );
    }
}

#[test]
fn banned_legacy_source_tool_fails() {
    let line = "limits: no arbitrary 3D scenes; no unrelated tooling. road. replayable road. Legacy source tool: walsy_world.";
    let out = run(&with_line(VALID, "limits:", line));
    assert!(!out.status.success());
    assert!(
        stdout(&out).lines().any(|l| l.ends_with(": walsy|_wor|ld")),
        "{}",
        stdout(&out)
    );
}

#[test]
fn banned_attribution_sentence_fails() {
    let line = "limits: no arbitrary 3D scenes; no unrelated tooling. road. replayable road. Attribution: derived from Python.";
    let out = run(&with_line(VALID, "limits:", line));
    assert!(!out.status.success());
    let text = stdout(&out);
    for token in [": attri|bution", ": deri|ved from", ": pyt|hon"] {
        assert!(text.lines().any(|l| l.ends_with(token)), "missing {token}: {text}");
    }
}

#[test]
fn word_boundary_non_hits_pass() {
    // `road.` is text; banned fragments embedded in longer words are not whole words.
    let line = "limits: road. replayable roadmap walsy_worldview.";
    let out = run(&with_line(VALID, "limits:", line));
    assert!(out.status.success(), "{}", stdout(&out));
    assert_eq!(stdout(&out), "CHECK PASS\n");
}

#[test]
fn negative_control_mutated_valid_fixture_fails_exactly() {
    let out = run(&with_line(VALID, "allowed-tools:", "allowed-tools: walsy_carve, walsy_preview, walsy_viewer"));
    assert!(!out.status.success());
    assert_eq!(
        stdout(&out),
        "CHECK FAIL\n- frontmatter: tool surface walsy_lint\n"
    );
}
