//! Deterministic four-view sheet presets. One parameter table, one small raster
//! primitive set, no external runtime; output goes straight to the carve engine.

use std::f64::consts::PI;

pub const PRESETS: &[&str] = &["apple", "banana", "orange"];

pub struct PresetDefaults {
    pub slices: i64,
    pub top_nose: &'static str,
    pub side_nose: &'static str,
    pub order: &'static str,
    pub colours: i64,
    pub shade: f64,
    pub rounded: bool,
    pub dark_thresh: i64,
    pub bg_tol: i64,
}

pub fn defaults(kind: &str) -> Option<PresetDefaults> {
    let d = match kind {
        "apple" => PresetDefaults {
            slices: 40,
            top_nose: "down",
            side_nose: "left",
            order: "reading",
            colours: 24,
            shade: 0.12,
            rounded: true,
            dark_thresh: 70,
            bg_tol: 28,
        },
        "banana" => PresetDefaults {
            slices: 14,
            top_nose: "down",
            side_nose: "left",
            order: "reading",
            colours: 24,
            shade: 0.2,
            rounded: false,
            dark_thresh: 70,
            bg_tol: 28,
        },
        "orange" => PresetDefaults {
            slices: 64,
            top_nose: "down",
            side_nose: "left",
            order: "reading",
            colours: 24,
            shade: 0.2,
            rounded: true,
            dark_thresh: 70,
            bg_tol: 28,
        },
        _ => return None,
    };
    Some(d)
}

pub fn sheet(kind: &str) -> Result<(usize, usize, Vec<[u8; 4]>), String> {
    match kind {
        "apple" => Ok(apple_sheet()),
        "banana" => Ok(banana_sheet()),
        "orange" => Ok(orange_sheet()),
        _ => Err(format!(
            "unknown sheet preset '{kind}' (choose from {})",
            PRESETS.join(", ")
        )),
    }
}

fn blank(w: usize, h: usize, bg: [u8; 4]) -> Vec<[u8; 4]> {
    vec![bg; w * h]
}

fn put(px: &mut [[u8; 4]], w: usize, h: usize, x: i32, y: i32, c: [u8; 4]) {
    if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
        px[y as usize * w + x as usize] = c;
    }
}

// Apple: four analytic primitives, four tone steps, one-pixel outline.
// Sheet contract carried by walsy-3d; primitives ported with fast-anything and cli-hub.
const A_C: i32 = 20;
const A_W: usize = 40;
const A_H: usize = 40;
const A_GAP: usize = 48;
const A_BG: [u8; 4] = [0, 0, 0, 0];
const A_OUTLINE: [u8; 4] = [38, 12, 16, 255];
const A_STEM: [u8; 4] = [92, 58, 34, 255];
const A_TONES: [[u8; 4]; 4] = [
    [128, 22, 34, 255],
    [184, 34, 46, 255],
    [228, 62, 64, 255],
    [255, 128, 108, 255],
];
const A_VIEWS: [&str; 4] = ["TOP", "SIDE", "FRONT", "BACK"];

pub fn apple_half_width(y: i32) -> f64 {
    let t = (y - A_C) as f64 / 18.0;
    if t.abs() > 1.0 {
        return 0.0;
    }
    17.0 * (1.0 - t * t).max(0.0).sqrt() * (1.0 + 0.10 * (PI * (t + 0.35)).sin())
}

pub fn apple_body(x: i32, y: i32, top: bool) -> bool {
    if top {
        let dx = (x - A_C) as f64;
        let dy = (y - A_C) as f64;
        let mut rr = dx * dx / (18.0 * 18.0) + dy * dy / (18.5 * 18.5);
        rr *= 1.0 + 0.035 * (4.0 * dy.atan2(dx)).cos();
        rr <= 1.0
    } else {
        apple_half_width(y) >= (x - A_C).abs() as f64
    }
}

pub fn apple_tone(x: i32, y: i32, top: bool) -> [u8; 4] {
    let dx = (x - A_C) as f64 / if top { 18.0 } else { 17.0 };
    let dy = (y - A_C) as f64 / if top { 18.5 } else { 18.0 };
    let lx = dx + 0.32;
    let ly = dy + 0.42;
    let v = 0.95 - (lx * lx * 0.9 + ly * ly * 1.05);
    if v > 0.58 {
        A_TONES[3]
    } else if v > 0.18 {
        A_TONES[2]
    } else if v > -0.30 {
        A_TONES[1]
    } else {
        A_TONES[0]
    }
}

pub fn apple_outline_cells(body: &[[bool; A_W]; A_H]) -> Vec<(usize, usize)> {
    let mut cells = Vec::new();
    for y in 0..A_H as i32 {
        for x in 0..A_W as i32 {
            if !body[y as usize][x as usize] {
                continue;
            }
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= A_W as i32 || ny >= A_H as i32 {
                        continue;
                    }
                    if !body[ny as usize][nx as usize] {
                        cells.push((nx as usize, ny as usize));
                    }
                }
            }
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

pub fn apple_draw_view(view: &str) -> Result<Vec<[u8; 4]>, String> {
    let index = A_VIEWS
        .iter()
        .position(|name| *name == view)
        .ok_or_else(|| format!("unknown apple view '{view}'"))?;
    let top = index == 0;
    let mut px = vec![A_BG; A_W * A_H];
    let mut body = [[false; A_W]; A_H];
    for y in 0..A_H as i32 {
        for x in 0..A_W as i32 {
            if apple_body(x, y, top) {
                body[y as usize][x as usize] = true;
                px[y as usize * A_W + x as usize] = apple_tone(x, y, top);
            }
        }
    }
    for (x, y) in apple_outline_cells(&body) {
        put(&mut px, A_W, A_H, x as i32, y as i32, A_OUTLINE);
    }
    if top {
        for y in 18..23 {
            for x in 18..23 {
                if (x - A_C) * (x - A_C) + (y - A_C) * (y - A_C) <= 4 {
                    put(&mut px, A_W, A_H, x, y, A_STEM);
                }
            }
        }
    } else {
        for y in 1..9 {
            for x in 19..22 {
                put(&mut px, A_W, A_H, x, y, A_STEM);
            }
        }
    }
    Ok(px)
}

fn apple_sheet() -> (usize, usize, Vec<[u8; 4]>) {
    let w = 4 * A_W + 3 * A_GAP;
    let mut px = blank(w, A_H, A_BG);
    for (view, name) in A_VIEWS.iter().enumerate() {
        let ox = (view * (A_W + A_GAP)) as i32;
        let local = apple_draw_view(name).expect("fixed view name");
        for y in 0..A_H {
            for x in 0..A_W {
                put(&mut px, w, A_H, ox + x as i32, y as i32, local[y * A_W + x]);
            }
        }
    }
    (w, A_H, px)
}

// Banana: compositional primitives over one parameter table.
type Rgb = [u8; 4];
const WHITE: Rgb = [255, 255, 255, 255];
const INK: Rgb = [58, 40, 12, 255];
const SHADOW: Rgb = [150, 83, 8, 255];
const MID: Rgb = [224, 152, 20, 255];
const LIGHT: Rgb = [246, 205, 84, 255];
const STEM: Rgb = [62, 44, 18, 255];

struct Panel {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

fn ellipse(px: &mut [Rgb], w: usize, h: usize, cx: f64, cy: f64, rx: f64, ry: f64, c: Rgb) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
        for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
            let dx = (x as f64 - cx) / rx;
            let dy = (y as f64 - cy) / ry;
            if dx * dx + dy * dy <= 1.0 {
                put(px, w, h, x, y, c);
            }
        }
    }
}

fn capsule(
    px: &mut [Rgb],
    w: usize,
    h: usize,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    r: f64,
    c: Rgb,
) {
    if r <= 0.0 {
        return;
    }
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len2 = dx * dx + dy * dy;
    for y in (y0.min(y1) - r).floor() as i32..=(y0.max(y1) + r).ceil() as i32 {
        for x in (x0.min(x1) - r).floor() as i32..=(x0.max(x1) + r).ceil() as i32 {
            let t = if len2 > 0.0 {
                ((x as f64 - x0) * dx + (y as f64 - y0) * dy) / len2
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            let ex = x as f64 - (x0 + t * dx);
            let ey = y as f64 - (y0 + t * dy);
            if ex * ex + ey * ey <= r * r {
                put(px, w, h, x, y, c);
            }
        }
    }
}

fn bezier2(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2], n: usize) -> Vec<[f64; 2]> {
    (0..n)
        .map(|i| {
            let t = i as f64 / (n - 1) as f64;
            let u = 1.0 - t;
            [
                u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
                u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
            ]
        })
        .collect()
}

fn ribbon<F: Fn(f64) -> f64>(
    px: &mut [Rgb],
    w: usize,
    h: usize,
    pts: &[[f64; 2]],
    radius: F,
    c: Rgb,
) {
    for i in 0..pts.len().saturating_sub(1) {
        let t = (i as f64 + 0.5) / (pts.len() - 1) as f64;
        capsule(
            px,
            w,
            h,
            pts[i][0],
            pts[i][1],
            pts[i + 1][0],
            pts[i + 1][1],
            radius(t),
            c,
        );
    }
}

fn shifted(pts: &[[f64; 2]], ox: f64, oy: f64) -> Vec<[f64; 2]> {
    pts.iter().map(|p| [p[0] + ox, p[1] + oy]).collect()
}

fn arc_radius(t: f64, r0: f64, r1: f64) -> f64 {
    r0 + (r1 - r0) * (PI * t).sin()
}

fn draw_top(px: &mut [Rgb], w: usize, h: usize) {
    let pts = bezier2([11.0, 4.0], [11.0 + 12.0 * 2.2, 36.0], [12.0, 68.0], 22);
    let rad = |t: f64| arc_radius(t, 4.2, 5.6);
    ribbon(px, w, h, &pts, |t| rad(t) + 1.1, INK);
    ribbon(px, w, h, &pts, rad, SHADOW);
    let hi = shifted(&pts, -1.4, -1.2);
    ribbon(px, w, h, &hi, |t| rad(t) - 1.8, LIGHT);
    let mid = shifted(&pts, -0.4, 0.5);
    ribbon(px, w, h, &mid, |t| rad(t) - 3.2, MID);
    capsule(
        px,
        w,
        h,
        pts[0][0],
        pts[0][1],
        pts[1][0],
        pts[1][1],
        rad(0.0) - 0.6,
        STEM,
    );
    let last = pts.len() - 1;
    capsule(
        px,
        w,
        h,
        pts[last - 1][0],
        pts[last - 1][1],
        pts[last][0],
        pts[last][1],
        rad(1.0) - 1.6,
        INK,
    );
}

fn draw_side(px: &mut [Rgb], w: usize, h: usize) {
    let pts = bezier2([3.0, 8.2], [36.0, 9.8], [69.0, 8.0], 26);
    let rad = |t: f64| arc_radius(t, 4.0, 5.0);
    ribbon(px, w, h, &pts, |t| rad(t) + 1.0, INK);
    ribbon(px, w, h, &pts, rad, SHADOW);
    let hi = shifted(&pts, 0.0, -1.3);
    ribbon(px, w, h, &hi, |t| rad(t) - 1.7, LIGHT);
    let mid = shifted(&pts, 0.0, 0.6);
    ribbon(px, w, h, &mid, |t| rad(t) - 3.0, MID);
    capsule(
        px,
        w,
        h,
        pts[0][0],
        pts[0][1],
        pts[1][0],
        pts[1][1],
        rad(0.0) - 0.4,
        STEM,
    );
    let last = pts.len() - 1;
    capsule(
        px,
        w,
        h,
        pts[last - 1][0],
        pts[last - 1][1],
        pts[last][0],
        pts[last][1],
        rad(1.0) - 0.9,
        STEM,
    );
}

fn draw_end(px: &mut [Rgb], w: usize, h: usize, flip: bool) {
    let cy = 7.0;
    ellipse(px, w, h, 18.0, cy, 17.0, 6.5, INK);
    ellipse(px, w, h, 18.0, cy, 16.0, 5.6, SHADOW);
    ellipse(
        px,
        w,
        h,
        18.0,
        cy + if flip { -1.2 } else { 1.2 },
        12.0,
        3.6,
        LIGHT,
    );
    ellipse(
        px,
        w,
        h,
        18.0,
        cy + if flip { -0.4 } else { 0.4 },
        8.0,
        2.2,
        MID,
    );
}

fn draw_panel(
    sheet: &mut [Rgb],
    sw: usize,
    sh: usize,
    panel: Panel,
    draw: impl FnOnce(&mut [Rgb], usize, usize),
) {
    let mut local = blank(panel.w, panel.h, WHITE);
    draw(&mut local, panel.w, panel.h);
    for y in 0..panel.h {
        for x in 0..panel.w {
            let c = local[y * panel.w + x];
            put(sheet, sw, sh, panel.x as i32 + x as i32, panel.y as i32 + y as i32, c);
        }
    }
}

fn banana_sheet() -> (usize, usize, Vec<[u8; 4]>) {
    let (w, h) = (404, 120);
    let mut px = blank(w, h, WHITE);
    draw_panel(
        &mut px, w, h,
        Panel { x: 40, y: 24, w: 36, h: 72 },
        draw_top,
    );
    draw_panel(
        &mut px, w, h,
        Panel { x: 124, y: 53, w: 72, h: 14 },
        draw_side,
    );
    draw_panel(
        &mut px, w, h,
        Panel { x: 244, y: 42, w: 36, h: 14 },
        |p, pw, ph| draw_end(p, pw, ph, false),
    );
    draw_panel(
        &mut px, w, h,
        Panel { x: 328, y: 42, w: 36, h: 14 },
        |p, pw, ph| draw_end(p, pw, ph, true),
    );
    (w, h, px)
}

// Orange: flat analytic disk, no bands; shading comes from the carve engine.
fn orange_sheet() -> (usize, usize, Vec<[u8; 4]>) {
    let (cell, gap, views) = (128usize, 44usize, 4usize);
    let w = cell * views + gap * (views - 1);
    let h = cell;
    let mut px = blank(w, h, WHITE);
    let base: Rgb = [232, 123, 18, 255];
    let stem: Rgb = [122, 62, 12, 255];
    for v in 0..views {
        let ox = v * (cell + gap);
        for y in 0..cell as i32 {
            for x in 0..cell as i32 {
                let dx = x - 64;
                let dy = y - 64;
                let body = dx * dx + dy * dy <= 60 * 60;
                let stick = (59..=69).contains(&x) && y >= 122;
                if body {
                    put(&mut px, w, h, ox as i32 + x, y, base);
                } else if stick {
                    put(&mut px, w, h, ox as i32 + x, y, stem);
                }
            }
        }
    }
    (w, h, px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_has_four_transparently_separated_views() {
        let (w, h, px) = sheet("apple").unwrap();
        assert_eq!((w, h), (304, 40));
        assert_eq!(px[40 * 20 + 40], A_BG);
        assert!(px.iter().any(|p| *p == A_TONES[3]));
    }

    #[test]
    fn banana_and_orange_keep_panel_geometry() {
        let (bw, bh, bp) = sheet("banana").unwrap();
        assert_eq!((bw, bh), (404, 120));
        assert_eq!(bp[24 * bw + 40], WHITE);
        assert!(bp.iter().any(|p| *p == LIGHT));
        let (ow, oh, op) = sheet("orange").unwrap();
        assert_eq!((ow, oh), (644, 128));
        assert_eq!(op[64 * ow + 64], [232, 123, 18, 255]);
        assert_eq!(op[64 * ow + 150], WHITE);
    }

    #[test]
    fn presets_drive_carve_defaults() {
        assert_eq!(defaults("apple").unwrap().slices, 40);
        assert_eq!(defaults("banana").unwrap().slices, 14);
        assert_eq!(defaults("orange").unwrap().slices, 64);
        assert!(defaults("nope").is_none());
        assert!(sheet("nope").is_err());
    }
}
