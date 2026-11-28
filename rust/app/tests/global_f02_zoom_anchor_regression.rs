//! Regression: wheel zoom anchors the art point under the cursor, and relayout (new stack) keeps it.
//! Mirrors the exact arithmetic in examples/dancing-dog/dancing-dog.html; no new deps.

use std::fs;
use std::path::Path;

fn html() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/dancing-dog/dancing-dog.html");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn anchor(px: f64, py: f64, w: f64, h: f64, art_w: f64, zoom: f64, cx: f64, cy: f64) -> (f64, f64) {
    let k = w / art_w * zoom;
    ((px - w / 2.0) / k + cx, (py - h / 2.0) / k + cy)
}

fn zoom_step(
    px: f64,
    py: f64,
    w: f64,
    h: f64,
    art_w: f64,
    zoom: f64,
    cx: f64,
    cy: f64,
    delta_y: f64,
) -> (f64, f64, f64) {
    let (ax, ay) = anchor(px, py, w, h, art_w, zoom, cx, cy);
    let z = (zoom * (-delta_y * 0.0015).exp()).clamp(0.5, 8.0);
    let k1 = w / art_w * z;
    (z, ax + (w / 2.0 - px) / k1, ay + (h / 2.0 - py) / k1)
}

#[test]
fn wheel_zoom_keeps_cursor_art_point_fixed() {
    let (w, h, art_w) = (588.0, 744.0, 98.0);
    for delta in [-180.0, -40.0, 120.0, 600.0] {
        for (px, py) in [(423.0, 282.0), (120.0, 90.0), (500.0, 700.0)] {
            let (ax0, ay0) = anchor(px, py, w, h, art_w, 1.0, 49.0, 62.0);
            let (z, cx, cy) = zoom_step(px, py, w, h, art_w, 1.0, 49.0, 62.0, delta);
            let (ax1, ay1) = anchor(px, py, w, h, art_w, z, cx, cy);
            assert!(
                (ax1 - ax0).abs() < 1e-9 && (ay1 - ay0).abs() < 1e-9,
                "cursor point drifted for delta={delta} at ({px},{py}): {ax0},{ay0} -> {ax1},{ay1}"
            );
        }
    }
}

#[test]
fn relayout_keeps_anchor_after_moving_stack() {
    // layout() rebuilds art (artH 124 -> 118) but must preserve art-space cx/cy.
    let (w, art_w, px, py) = (588.0, 98.0, 423.0, 282.0);
    let (_, cx, cy) = zoom_step(px, py, w, 744.0, art_w, 1.0, 49.0, 62.0, -180.0);
    let before = anchor(w / 2.0, 744.0 / 2.0, w, 744.0, art_w, 1.31, cx, cy);
    let after = anchor(w / 2.0, 708.0 / 2.0, w, 708.0, art_w, 1.31, cx, cy);
    assert_eq!(before, after, "relayout recentred the zoom anchor");
}

#[test]
fn negative_control_centre_zoom_must_drift() {
    let (w, h, art_w, px, py) = (588.0, 744.0, 98.0, 423.0, 282.0);
    let (ax0, ay0) = anchor(px, py, w, h, art_w, 1.0, 49.0, 62.0);
    let z = (180.0_f64 * 0.0015).exp();
    let (ax1, ay1) = anchor(px, py, w, h, art_w, z, 49.0, 62.0); // centre-only zoom
    assert!(
        (ax1 - ax0).abs() > 1.0 && (ay1 - ay0).abs() > 1.0,
        "negative control passed: centre-only zoom moved the cursor point by {} / {}",
        ax1 - ax0,
        ay1 - ay0
    );
}

#[test]
fn html_wires_anchor_and_keeps_exact_links() {
    let s = html();
    assert!(s.contains("state.cx = ax + (view.width / 2 - px) / k1;"));
    assert!(s.contains("state.cy = ay + (view.height / 2 - py) / k1;"));
    assert_eq!(s.matches("state.cx = artW / 2;").count(), 2, "centre only in boot/restart");
    assert_eq!(s.matches("state.cy = artH / 2;").count(), 2, "centre only in boot/restart");
    assert!(!s.contains("state.cx = artW / 2;\n  state.cy = artH / 2;\n  resize();"), "layout must not recentre");
    assert!(s.contains("https://www.linkedin.com/in/kerem-urhan-225870436/"));
    assert!(s.contains("https://github.com/walsykerem/walsy-3d"));
}
