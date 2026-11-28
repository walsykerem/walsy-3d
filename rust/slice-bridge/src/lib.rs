//! Opt-in Rust bridge packed alpha runs.

use pyo3::prelude::*;

#[pyfunction]
#[pyo3(signature = (vol, min_run = 3))]
fn thin_runs(vol: &Bound<'_, PyAny>, min_run: i64) -> PyResult<Vec<(i64, i64)>> {
    let depth = vol.len()?;
    if depth == 0 {
        return Ok(Vec::new());
    }
    let first = vol.get_item(0)?;
    let h = first.len()?;
    if h == 0 {
        return Ok(Vec::new());
    }
    let first_px = first.get_item(0)?;
    let w = first_px.len()?;
    if w == 0 {
        return Ok(Vec::new());
    }

    let mut stack = Vec::with_capacity(depth * h * w);
    for z in 0..depth {
        let plane = vol.get_item(z)?;
        let mut alpha = vec![false; h * w];
        for y in 0..h {
            let row = plane.get_item(y)?;
            for x in 0..w {
                let rgba = row.get_item(x)?;
                let a: u8 = rgba.get_item(3)?.extract()?;
                alpha[y * w + x] = a > 0;
            }
        }

        let mut block = vec![false; h * w];
        if h > 1 && w > 1 {
            let bh = h - 1;
            let bw = w - 1;
            let mut b = vec![false; bh * bw];
            for y in 0..bh {
                for x in 0..bw {
                    b[y * bw + x] = alpha[y * w + x]
                        && alpha[(y + 1) * w + x]
                        && alpha[y * w + x + 1]
                        && alpha[(y + 1) * w + x + 1];
                }
            }
            for dy in 0..2 {
                for dx in 0..2 {
                    for y in 0..bh {
                        let dst = (y + dy) * w + dx;
                        let src = y * bw;
                        for x in 0..bw {
                            block[dst + x] |= b[src + x];
                        }
                    }
                }
            }
        }

        let mut seen = vec![false; h * w];
        let mut found = false;
        let mut todo = Vec::new();
        for y0 in 0..h {
            for x0 in 0..w {
                let i0 = y0 * w + x0;
                if !alpha[i0] || seen[i0] || found {
                    continue;
                }
                seen[i0] = true;
                todo.clear();
                todo.push((y0, x0));
                let mut solid = false;
                while let Some((y, x)) = todo.pop() {
                    solid |= block[y * w + x];
                    if y + 1 < h {
                        let i = (y + 1) * w + x;
                        if alpha[i] && !seen[i] {
                            seen[i] = true;
                            todo.push((y + 1, x));
                        }
                    }
                    if y > 0 {
                        let i = (y - 1) * w + x;
                        if alpha[i] && !seen[i] {
                            seen[i] = true;
                            todo.push((y - 1, x));
                        }
                    }
                    if x + 1 < w {
                        let i = y * w + x + 1;
                        if alpha[i] && !seen[i] {
                            seen[i] = true;
                            todo.push((y, x + 1));
                        }
                    }
                    if x > 0 {
                        let i = y * w + x - 1;
                        if alpha[i] && !seen[i] {
                            seen[i] = true;
                            todo.push((y, x - 1));
                        }
                    }
                }
                found = !solid;
                if found {
                    break;
                }
            }
            if found {
                break;
            }
        }
        stack.push(found);
    }

    let mut runs = Vec::new();
    let mut start: Option<i64> = None;
    for z in 0..=depth {
        let t = z < depth && stack[z];
        if t && start.is_none() {
            start = Some(z as i64);
        } else if !t && start.is_some() {
            let s = start.unwrap();
            if z as i64 - s >= min_run {
                runs.push((s, z as i64 - 1));
            }
            start = None;
        }
    }
    Ok(runs)
}

#[pymodule]
fn fa_scripts_views_to_slices_py_thin_runs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(thin_runs, m)?)?;
    Ok(())
}
