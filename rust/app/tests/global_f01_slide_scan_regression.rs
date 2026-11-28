//! Slide/scan stability guard for the browser viewer template.
//! Keep the checks source-based: the template is HTML/JS, not a Rust module.

const VIEWER: &str = include_str!("../../scripts-rs/src/viewer.html");

fn failures(html: &str) -> Vec<&'static str> {
    let mut out = Vec::new();

    if !html.contains("function scanIndex(n){") {
        out.push("angle-derived scan index helper");
    }
    if !html.contains("return Math.floor((((state.angle % 360) + 360) % 360) * n / 360) % n;") {
        out.push("scan index derived from view angle");
    }
    if html.contains("state.scan") {
        out.push("no competing scan timer or phase");
    }
    if !html.contains("const off = Math.round(z * S) - Math.round(mid * S);") {
        out.push("pixel-snapped slice lift");
    }
    if !html.contains("const off = Math.round(z * S) - Math.round(half * S);") {
        out.push("pixel-snapped ground lift");
    }
    if !html.contains("Object.assign(state, DEFAULTS, { slices: payloadSlices });") {
        out.push("restart keeps the angle-derived scan phase");
    }
    if !html.contains("https://www.linkedin.com/in/kerem-urhan-225870436/")
        || !html.contains("https://github.com/walsykerem/walsy-3d")
    {
        out.push("exact links preserved");
    }

    if let Some(start) = html.find("els.stage.addEventListener('wheel'") {
        let end = html[start..]
            .find("}, { passive: false });")
            .map(|i| start + i)
            .unwrap_or(html.len());
        if html[start..end].contains("scan") {
            out.push("wheel zoom leaves scan phase untouched");
        }
    } else {
        out.push("wheel zoom handler present");
    }

    out
}

#[test]
fn fixed_viewer_passes_every_guard() {
    let fails = failures(VIEWER);
    assert!(fails.is_empty(), "viewer guards failed: {fails:?}");
}

#[test]
fn negative_control_old_wobbly_viewer_fails() {
    let old = VIEWER
        .replace(
            "const off = Math.round(z * S) - Math.round(mid * S);",
            "const off = (z - mid) * S;",
        )
        .replace(
            "const off = Math.round(z * S) - Math.round(half * S);",
            "const off = (z - half) * S;",
        )
        .replace(
            "return Math.floor((((state.angle % 360) + 360) % 360) * n / 360) % n;",
            "return state.scan;",
        )
        .replace(
            "Object.assign(state, DEFAULTS, { slices: payloadSlices });",
            "Object.assign(state, DEFAULTS, { slices: payloadSlices, scan: 0 });",
        );

    let fails = failures(&old);
    for expected in [
        "pixel-snapped slice lift",
        "pixel-snapped ground lift",
        "scan index derived from view angle",
        "no competing scan timer or phase",
        "restart keeps the angle-derived scan phase",
    ] {
        assert!(fails.contains(&expected), "negative control missed {expected}: {fails:?}");
    }
}
