use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const W: usize = 8;
const H: usize = 4;
const N: usize = 4;
const NOSE: [u8; 4] = [240, 40, 40, 255];
const ELEVATIONS: [u32; 5] = [15, 30, 45, 60, 75];

const fn body(z: usize) -> [u8; 4] {
    [40 + (40 * z) as u8, 90, 160, 255]
}

const COLOURS: [[u8; 4]; N] = [body(0), body(1), body(2), body(3)];

fn pixel(p: [u8; 4]) -> Value {
    json!([p[0], p[1], p[2], p[3]])
}

fn volume() -> Value {
    Value::Array(
        (0..N)
            .map(|z| {
                Value::Array(
                    (0..H)
                        .map(|_| Value::Array((0..W).map(|_| pixel(body(z))).collect()))
                        .collect(),
                )
            })
            .collect(),
    )
}

fn one_slice_volume() -> Value {
    Value::Array(vec![Value::Array(
        (0..H)
            .map(|_| {
                Value::Array(
                    (0..W)
                        .map(|x| pixel(if x + 1 == W { NOSE } else { body(0) }))
                        .collect(),
                )
            })
            .collect(),
    )])
}

fn stripe_volume() -> Value {
    Value::Array(
        (0..N)
            .map(|z| {
                Value::Array(
                    (0..H)
                        .map(|_| {
                            Value::Array(
                                (0..W)
                                    .map(|x| {
                                        pixel(if (2 * z..2 * z + 2).contains(&x) {
                                            body(z)
                                        } else {
                                            [0, 0, 0, 0]
                                        })
                                    })
                                    .collect(),
                            )
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

struct View {
    rows: usize,
    cols: usize,
    px: Vec<Vec<[u8; 4]>>,
}

fn byte(cell: &[Value], at: usize) -> u8 {
    cell.get(at)
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .try_into()
        .unwrap()
}

fn render(vol: &Value, angle: f64, squash: f64, spacing: f64) -> View {
    let plane = scripts_rs::engine::render_stack_image(
        vol.clone(), angle, squash, spacing, false, false,
    )
    .expect("real engine must render the fixture");
    let rows = plane.as_array().expect("plane rows");
    let px = rows
        .iter()
        .map(|row| {
            row.as_array()
                .expect("plane row")
                .iter()
                .map(|cell| {
                    let c = cell.as_array().expect("rgba");
                    [byte(c, 0), byte(c, 1), byte(c, 2), byte(c, 3)]
                })
                .collect()
        })
        .collect();
    View {
        rows: rows.len(),
        cols: rows.first().and_then(Value::as_array).map_or(0, Vec::len),
        px,
    }
}

fn bbox(view: &View) -> (usize, usize, usize, usize) {
    let (mut min_x, mut min_y) = (usize::MAX, usize::MAX);
    let (mut max_x, mut max_y) = (0, 0);
    for y in 0..view.rows {
        for x in 0..view.cols {
            if view.px[y][x][3] > 0 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    assert_ne!(min_x, usize::MAX, "render is empty");
    (min_x, min_y, max_x, max_y)
}

#[derive(Clone, Copy, Debug)]
struct Band {
    top: usize,
    bottom: usize,
}

fn band(view: &View, colour: [u8; 4]) -> Option<Band> {
    let mut found = None;
    for y in 0..view.rows {
        if view.px[y].iter().any(|&p| p == colour) {
            let b = found.get_or_insert(Band { top: y, bottom: y });
            b.top = b.top.min(y);
            b.bottom = b.bottom.max(y);
        }
    }
    found
}

#[derive(Debug)]
struct Analysis {
    bands: Vec<Band>,
    gaps: Vec<usize>,
}

fn analyse(view: &View, colours: &[[u8; 4]], spacing: f64) -> Result<Analysis, String> {
    let mut bands: Vec<Band> = colours
        .iter()
        .map(|&c| {
            band(view, c)
                .ok_or_else(|| format!("spacing={spacing:?} missing slice colour {c:?}"))
        })
        .collect::<Result<_, _>>()?;
    bands.sort_by_key(|b| b.top);
    let gaps: Vec<usize> = bands
        .windows(2)
        .map(|w| (w[1].top as isize - w[0].bottom as isize - 1).max(0) as usize)
        .collect();
    if gaps.iter().any(|&g| g == 0) {
        return Err(format!("spacing={spacing:?} gaps={gaps:?} zero-gap wall"));
    }
    Ok(Analysis { bands, gaps })
}

fn centroid(view: &View, colour: [u8; 4]) -> (f64, f64) {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
    for y in 0..view.rows {
        for x in 0..view.cols {
            if view.px[y][x] == colour {
                sx += x as f64;
                sy += y as f64;
                n += 1.0;
            }
        }
    }
    assert!(n > 0.0, "missing colour {colour:?}");
    (sx / n, sy / n)
}

fn temp_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("walsy-t07-{}-{nanos}", std::process::id()))
}

#[test]
fn elevation_endpoints_match_slice_geometry() {
    let vol = volume();

    let flat = render(&vol, 0.0, 1.0, 0.0);
    let (min_x, min_y, max_x, max_y) = bbox(&flat);
    assert_eq!(max_x - min_x + 1, W, "elevation 0 must be one slice wide");
    assert_eq!(max_y - min_y + 1, H, "elevation 0 must be flat height");

    let upright = render(&vol, 90.0, 1.0, 0.0);
    let (min_x, min_y, max_x, max_y) = bbox(&upright);
    assert_eq!(max_x - min_x + 1, H, "elevation 90 width must equal sliceHeight");
    assert_eq!(max_y - min_y + 1, W, "elevation 90 height must equal sliceWidth");
}

#[test]
fn intermediate_elevations_separate_pixel_snapped_layers() {
    let vol = volume();
    let mut previous_gap = 0;

    for &elevation in &ELEVATIONS {
        let spacing = (elevation as f64 / 90.0 * 60.0).ceil();
        let view = render(&vol, elevation as f64, 1.0, spacing);
        let analysis = analyse(&view, &COLOURS, spacing)
            .unwrap_or_else(|e| panic!("elevation {elevation}: {e}"));

        assert_eq!(analysis.bands.len(), N);
        let lifts: Vec<usize> = analysis
            .bands
            .windows(2)
            .map(|w| w[1].top - w[0].top)
            .collect();
        assert_eq!(
            lifts,
            vec![spacing as usize; N - 1],
            "elevation {elevation}: lifts must land on whole pixels"
        );
        assert!(
            analysis.gaps[0] > previous_gap,
            "elevation {elevation}: separation must grow: {} <= {previous_gap}",
            analysis.gaps[0]
        );
        previous_gap = analysis.gaps[0];
    }
}

#[test]
fn reversing_angle_holds_heading() {
    let vol = one_slice_volume();

    let plus = render(&vol, 45.0, 1.0, 0.0);
    let (nose_x, nose_y) = centroid(&plus, NOSE);
    let (body_x, body_y) = centroid(&plus, body(0));
    assert!(nose_x > body_x, "+45 nose must stay on the +x side");
    assert!(nose_y > body_y, "+45 nose must tilt down");

    let minus = render(&vol, -45.0, 1.0, 0.0);
    let (nose_x, nose_y) = centroid(&minus, NOSE);
    let (body_x, body_y) = centroid(&minus, body(0));
    assert!(nose_x > body_x, "-45 must keep the same heading");
    assert!(nose_y < body_y, "-45 must mirror the tilt, not the heading");
}

#[test]
fn negative_control_zero_spacing_flat_wall_is_rejected_with_exact_values() {
    let path = temp_path();
    let fixture = json!({
        "angle": 0.0,
        "squash": 1.0,
        "spacing": 0.0,
        "volume": stripe_volume(),
    });
    fs::write(&path, serde_json::to_vec(&fixture).unwrap()).unwrap();

    let read: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let spacing = read["spacing"].as_f64().unwrap();
    let view = render(
        &read["volume"],
        read["angle"].as_f64().unwrap(),
        read["squash"].as_f64().unwrap(),
        spacing,
    );
    let error = analyse(&view, &COLOURS, spacing).unwrap_err();
    assert_eq!(error, "spacing=0.0 gaps=[0, 0, 0] zero-gap wall");

    fs::remove_file(&path).unwrap();
}
