use serde_json::{Value, json};
use scripts_rs::engine::{elevation_projection, render_stack_at_elevation, render_stack_image};

const W: usize = 8;
const H: usize = 4;
const N: usize = 4;

fn body(z: usize) -> [u8; 4] {
    [40 + (40 * z) as u8, 90, 160, 255]
}

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

fn view(v: &Value) -> View {
    let rows = v.as_array().expect("plane rows");
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

fn render(v: &Value, angle: f64, elevation: f64) -> View {
    view(&render_stack_at_elevation(v.clone(), angle, elevation, false, false).unwrap())
}

fn bbox(v: &View) -> (usize, usize, usize, usize) {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (usize::MAX, usize::MAX, 0, 0);
    for y in 0..v.rows {
        for x in 0..v.cols {
            if v.px[y][x][3] > 0 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    assert_ne!(min_x, usize::MAX, "empty render");
    (min_x, min_y, max_x, max_y)
}

fn rows_of(v: &View, colour: [u8; 4]) -> Vec<usize> {
    (0..v.rows)
        .filter(|&y| v.px[y].iter().any(|&p| p == colour))
        .collect()
}

#[test]
fn projection_is_orthographic_and_clamped() {
    assert_eq!(elevation_projection(0.0), (0.0, 1.0));
    let (top_sq, top_sp) = elevation_projection(90.0);
    assert!((top_sq - 1.0).abs() < 1e-9);
    assert!(top_sp.abs() < 1e-9);
    assert_eq!(elevation_projection(-30.0), elevation_projection(0.0));
    assert_eq!(elevation_projection(120.0), elevation_projection(90.0));
    for e in [10.0, 35.0, 49.0, 75.0] {
        let (sq, sp) = elevation_projection(e);
        assert!((sq * sq + sp * sp - 1.0).abs() < 1e-9);
    }
}

#[test]
fn elevation_endpoints_match_slice_geometry() {
    let vol = volume();
    let flat = render(&vol, 0.0, 0.0);
    let (x0, _, x1, _) = bbox(&flat);
    assert_eq!(x1 - x0 + 1, W, "elevation 0 must be flat-width");

    let upright = render(&vol, 90.0, 90.0);
    let (x0, _, x1, _) = bbox(&upright);
    assert_eq!(x1 - x0 + 1, H, "elevation 90 width must equal sliceHeight");
}

#[test]
fn intermediate_elevations_keep_layers_separate_without_stripes() {
    let vol = stripe_volume();
    for &e in &[15.0, 30.0, 45.0, 60.0, 75.0] {
        let v = render(&vol, 0.0, e);
        let mut tops = Vec::new();
        for z in 0..N {
            let rows = rows_of(&v, body(z));
            assert!(!rows.is_empty(), "elevation {e}: slice {z} vanished");
            assert_eq!(
                rows.last().unwrap() - rows.first().unwrap() + 1,
                rows.len(),
                "elevation {e}: slice {z} split into stripes"
            );
            tops.push(rows[0]);
        }
        assert!(
            tops.windows(2).all(|w| w[0] >= w[1]) && tops[0] > tops[N - 1],
            "elevation {e}: layers merged into a flat wall: {tops:?}"
        );
    }
}

#[test]
fn edge_on_keeps_off_center_pixels() {
    // Opaque pixels only in the top source row, away from the middle row.
    let vol = Value::Array(
        (0..N)
            .map(|z| {
                Value::Array(
                    (0..H)
                        .map(|y| {
                            Value::Array(
                                (0..W)
                                    .map(|_| {
                                        pixel(if y == 0 { body(z) } else { [0, 0, 0, 0] })
                                    })
                                    .collect(),
                            )
                        })
                        .collect(),
                )
            })
            .collect(),
    );
    let flat = render(&vol, 0.0, 0.0);
    for z in 0..N {
        assert!(
            !rows_of(&flat, body(z)).is_empty(),
            "elevation 0: off-center slice {z} disappeared"
        );
    }
}

#[test]
fn negative_control_zero_spacing_flat_wall_fails() {
    let vol = volume();
    let flat = view(
        &render_stack_image(vol, 0.0, 1.0, 0.0, false, false).unwrap(),
    );
    for z in 0..N - 1 {
        assert!(
            rows_of(&flat, body(z)).is_empty(),
            "zero spacing must hide lower slices under the wall"
        );
    }
    assert!(
        !rows_of(&flat, body(N - 1)).is_empty(),
        "zero spacing must leave a single top layer"
    );
}
