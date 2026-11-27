#![forbid(unsafe_code)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_arguments
)]

use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_i64() || n.is_u64() => "int",
        Value::Number(_) => "float",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn scalar(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::Bool(true) => 1.0,
        Value::Bool(false) => 0.0,
        _ => 0.0,
    }
}

fn dimensions(v: &Value) -> Option<Vec<usize>> {
    match v {
        Value::Number(_) => Some(Vec::new()),
        Value::Array(a) => {
            let inner = dimensions(a.first()?)?;
            if a.iter().all(|x| dimensions(x).as_ref() == Some(&inner)) {
                let mut d = Vec::with_capacity(inner.len() + 1);
                d.push(a.len());
                d.extend(inner);
                Some(d)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn shape_error(v: &Value) -> String {
    format!(
        "AttributeError: '{}' object has no attribute 'shape'",
        type_name(v)
    )
}

fn interface_error(v: &Value) -> String {
    format!(
        "AttributeError: '{}' object has no attribute '__array_interface__'",
        type_name(v)
    )
}

fn index_error(v: &Value, at: &str) -> String {
    match v {
        Value::Object(_) => format!("KeyError: ({at}, 3)"),
        Value::Array(_) => "list indices must be integers or slices, not tuple".into(),
        Value::String(_) => "string indices must be integers, not 'tuple'".into(),
        _ => format!("'{}' object is not subscriptable", type_name(v)),
    }
}

fn not_iterable(v: &Value) -> String {
    format!("'{}' object is not iterable", type_name(v))
}

fn view_list(views: &Value) -> Result<&[Value], String> {
    views
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| not_iterable(views))
}

fn round_even(x: f64) -> i64 {
    x.round_ties_even() as i64
}

fn ratio_round(numerator: usize, denominator: usize) -> usize {
    let d = denominator.max(1);
    let q = numerator / d;
    let twice = 2 * (numerator % d);
    if twice > d || (twice == d && q % 2 == 1) {
        q + 1
    } else {
        q
    }
}

fn clamp_to(value: i64, low: usize, high: usize) -> usize {
    if low > high {
        return low;
    }
    if value < low as i64 {
        low
    } else if value > high as i64 {
        high
    } else {
        value as usize
    }
}

fn source_index(i: usize, source: usize, target: usize) -> usize {
    let source = source.max(1);
    let target = target.max(1);
    i.saturating_mul(source)
        .saturating_add(source / 2)
        .div_euclid(target)
        .min(source - 1)
}

#[derive(Clone)]
struct Plane {
    rows: usize,
    cols: usize,
    bands: usize,
    data: Vec<u8>,
}

impl Plane {
    fn new(rows: usize, cols: usize, bands: usize) -> Self {
        Self {
            rows,
            cols,
            bands,
            data: vec![0; rows * cols * bands],
        }
    }

    fn offset(&self, y: usize, x: usize, ch: usize) -> usize {
        (y * self.cols + x) * self.bands + ch
    }

    fn get(&self, y: usize, x: usize, ch: usize) -> u8 {
        self.data[self.offset(y, x, ch)]
    }

    fn set(&mut self, y: usize, x: usize, ch: usize, value: u8) {
        let at = self.offset(y, x, ch);
        self.data[at] = value;
    }

    fn alpha(&self, y: usize, x: usize) -> u8 {
        if self.bands > 3 { self.get(y, x, 3) } else { 255 }
    }

    fn rgba(&self, y: usize, x: usize) -> [u8; 4] {
        let mut p = [255; 4];
        for ch in 0..self.bands.min(4) {
            p[ch] = self.get(y, x, ch);
        }
        p
    }

    fn paint(&mut self, y: usize, x: usize, p: [u8; 4]) {
        for ch in 0..self.bands.min(4) {
            self.set(y, x, ch, p[ch]);
        }
    }

    fn json(&self) -> Value {
        let mut rows = Vec::with_capacity(self.rows);
        for y in 0..self.rows {
            let mut cols = Vec::with_capacity(self.cols);
            for x in 0..self.cols {
                let p: Vec<Value> = (0..self.bands)
                    .map(|ch| Value::from(self.get(y, x, ch)))
                    .collect();
                cols.push(Value::Array(p));
            }
            rows.push(Value::Array(cols));
        }
        Value::Array(rows)
    }

    fn flip_columns(&self) -> Self {
        let mut out = self.clone();
        for y in 0..self.rows {
            for x in 0..self.cols {
                for ch in 0..self.bands {
                    out.set(y, x, ch, self.get(y, self.cols - 1 - x, ch));
                }
            }
        }
        out
    }

    fn flip_rows(&self) -> Self {
        let mut out = self.clone();
        for y in 0..self.rows {
            for x in 0..self.cols {
                for ch in 0..self.bands {
                    out.set(y, x, ch, self.get(self.rows - 1 - y, x, ch));
                }
            }
        }
        out
    }

    fn crop(&self, y0: usize, x0: usize, y1: usize, x1: usize) -> Self {
        let rows = y1.saturating_sub(y0);
        let cols = x1.saturating_sub(x0);
        let mut out = Self::new(rows, cols, self.bands);
        for y in 0..rows {
            for x in 0..cols {
                for ch in 0..self.bands {
                    out.set(y, x, ch, self.get(y0 + y, x0 + x, ch));
                }
            }
        }
        out
    }
}

fn plane_from(v: &Value) -> Result<Plane, String> {
    let d = dimensions(v).ok_or_else(|| shape_error(v))?;
    if d.len() != 3 || d[2] < 3 || d[0] == 0 || d[1] == 0 {
        return Err(shape_error(v));
    }
    let mut im = Plane::new(d[0], d[1], d[2]);
    for y in 0..d[0] {
        let row = v[y].as_array().ok_or_else(|| shape_error(v))?;
        for x in 0..d[1] {
            let px = row[x].as_array().ok_or_else(|| shape_error(v))?;
            for ch in 0..d[2] {
                im.set(y, x, ch, scalar(&px[ch]) as u8);
            }
        }
    }
    Ok(im)
}

fn resize(im: &Plane, cols: i64, rows: i64) -> Plane {
    let cols = cols.max(1) as usize;
    let rows = rows.max(1) as usize;
    let mut out = Plane::new(rows, cols, im.bands);
    for y in 0..rows {
        let sy = source_index(y, im.rows, rows);
        for x in 0..cols {
            let sx = source_index(x, im.cols, cols);
            for ch in 0..im.bands {
                out.set(y, x, ch, im.get(sy, sx, ch));
            }
        }
    }
    out
}

#[derive(Clone)]
struct Block {
    depth: usize,
    rows: usize,
    cols: usize,
    cells: Vec<[u8; 4]>,
}

impl Block {
    fn new(depth: usize, rows: usize, cols: usize) -> Self {
        Self {
            depth,
            rows,
            cols,
            cells: vec![[0; 4]; depth * rows * cols],
        }
    }

    fn offset(&self, z: usize, y: usize, x: usize) -> usize {
        (z * self.rows + y) * self.cols + x
    }

    fn at(&self, z: usize, y: usize, x: usize) -> [u8; 4] {
        self.cells[self.offset(z, y, x)]
    }

    fn put(&mut self, z: usize, y: usize, x: usize, p: [u8; 4]) {
        let at = self.offset(z, y, x);
        self.cells[at] = p;
    }

    fn json(&self) -> Value {
        let mut planes = Vec::with_capacity(self.depth);
        for z in 0..self.depth {
            let mut rows = Vec::with_capacity(self.rows);
            for y in 0..self.rows {
                let mut cols = Vec::with_capacity(self.cols);
                for x in 0..self.cols {
                    let p = self.at(z, y, x);
                    cols.push(Value::Array(p.iter().map(|v| Value::from(*v)).collect()));
                }
                rows.push(Value::Array(cols));
            }
            planes.push(Value::Array(rows));
        }
        Value::Array(planes)
    }
}

fn block_from(v: &Value) -> Result<Block, String> {
    let d = dimensions(v).ok_or_else(|| shape_error(v))?;
    if d.len() != 4 || d[3] < 4 {
        return Err(shape_error(v));
    }
    let mut out = Block::new(d[0], d[1], d[2]);
    for z in 0..d[0] {
        let plane = v[z].as_array().ok_or_else(|| shape_error(v))?;
        for y in 0..d[1] {
            let row = plane[y].as_array().ok_or_else(|| shape_error(v))?;
            for x in 0..d[2] {
                let p = row[x].as_array().ok_or_else(|| shape_error(v))?;
                let mut cell = [0; 4];
                for ch in 0..4 {
                    cell[ch] = scalar(&p[ch]) as u8;
                }
                out.put(z, y, x, cell);
            }
        }
    }
    Ok(out)
}

pub fn remove_background(rgba: serde_json::Value, tol: i64) -> Result<serde_json::Value, String> {
    let mut im = plane_from(&rgba)?;
    let total = (im.rows * im.cols) as f64;
    let mut transparent_count = 0usize;
    for y in 0..im.rows {
        for x in 0..im.cols {
            if im.alpha(y, x) < 128 {
                transparent_count += 1;
            }
        }
    }
    let transparent = transparent_count as f64 / total > 0.05;
    if transparent {
        for y in 0..im.rows {
            for x in 0..im.cols {
                if im.bands > 3 {
                    im.set(y, x, 3, if im.alpha(y, x) >= 128 { 255 } else { 0 });
                }
            }
        }
        return Ok(im.json());
    }

    let corners = [
        (0, 0),
        (0, im.cols - 1),
        (im.rows - 1, 0),
        (im.rows - 1, im.cols - 1),
    ];
    let mut background = [0.0; 3];
    for ch in 0..3 {
        let mut values: Vec<f64> = corners
            .iter()
            .map(|&(y, x)| f64::from(im.get(y, x, ch)))
            .collect();
        values.sort_by(f64::total_cmp);
        background[ch] = (values[1] + values[2]) / 2.0;
    }

    let mut near = vec![false; im.rows * im.cols];
    for y in 0..im.rows {
        for x in 0..im.cols {
            let d = (0..3)
                .map(|ch| (f64::from(im.get(y, x, ch)) - background[ch]).abs())
                .fold(0.0_f64, f64::max);
            near[y * im.cols + x] = d <= tol as f64;
        }
    }

    let mut reached = vec![false; im.rows * im.cols];
    let mut queue = VecDeque::new();
    let mut seed = |y: usize, x: usize| {
        let at = y * im.cols + x;
        if near[at] && !reached[at] {
            reached[at] = true;
            queue.push_back((y, x));
        }
    };
    for x in 0..im.cols {
        seed(0, x);
        seed(im.rows - 1, x);
    }
    for y in 0..im.rows {
        seed(y, 0);
        seed(y, im.cols - 1);
    }
    while let Some((y, x)) = queue.pop_front() {
        let neighbours = [
            (y + 1 < im.rows).then_some((y + 1, x)),
            y.checked_sub(1).map(|n| (n, x)),
            (x + 1 < im.cols).then_some((y, x + 1)),
            x.checked_sub(1).map(|n| (y, n)),
        ];
        for (ny, nx) in neighbours.into_iter().flatten() {
            let at = ny * im.cols + nx;
            if near[at] && !reached[at] {
                reached[at] = true;
                queue.push_back((ny, nx));
            }
        }
    }
    for y in 0..im.rows {
        for x in 0..im.cols {
            if im.bands > 3 {
                im.set(y, x, 3, if reached[y * im.cols + x] { 0 } else { 255 });
            }
        }
    }
    Ok(im.json())
}

fn trim_view(im: &Plane) -> Plane {
    let mut top = im.rows;
    let mut bottom = 0;
    let mut left = im.cols;
    let mut right = 0;
    for y in 0..im.rows {
        for x in 0..im.cols {
            if im.alpha(y, x) >= 128 {
                top = top.min(y);
                bottom = bottom.max(y);
                left = left.min(x);
                right = right.max(x);
            }
        }
    }
    if bottom < top || right < left {
        let mut empty = Plane::new(im.rows, im.cols, im.bands);
        if empty.bands > 3 {
            empty.data.fill(0);
        }
        return empty;
    }
    let mut cut = im.crop(top, left, bottom + 1, right + 1);
    for y in 0..cut.rows {
        for x in 0..cut.cols {
            if cut.bands > 3 {
                cut.set(y, x, 3, if cut.alpha(y, x) >= 128 { 255 } else { 0 });
            }
        }
    }
    cut
}

fn sheet_views(sheet: &Value, order: &str) -> Result<Vec<Value>, String> {
    let im = plane_from(sheet).map_err(|_| index_error(sheet, "Ellipsis"))?;
    let (h, w) = (im.rows, im.cols);
    let grow_steps = round_even((h.min(w) as f64 / 260.0).clamp(2.0, 6.0)).max(1) as usize;
    let mut grown = vec![false; h * w];
    for y in 0..h {
        for x in 0..w {
            grown[y * w + x] = im.alpha(y, x) >= 128;
        }
    }
    for _ in 0..grow_steps {
        let previous = grown.clone();
        for y in 0..h {
            for x in 0..w {
                let at = y * w + x;
                grown[at] = previous[at]
                    || (y + 1 < h && previous[at + w])
                    || (y > 0 && previous[at - w])
                    || (x + 1 < w && previous[at + 1])
                    || (x > 0 && previous[at - 1]);
            }
        }
    }

    let mut visited = vec![false; h * w];
    let mut boxes: Vec<(usize, usize, usize, usize)> = Vec::new();
    for y0 in 0..h {
        for x0 in 0..w {
            let root = y0 * w + x0;
            if !grown[root] || visited[root] {
                continue;
            }
            visited[root] = true;
            let mut stack = vec![(y0, x0)];
            let (mut min_x, mut max_x, mut min_y, mut max_y, mut count) = (x0, x0, y0, y0, 0);
            while let Some((y, x)) = stack.pop() {
                count += 1;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                for (ny, nx) in [
                    (y + 1 < h).then_some((y + 1, x)),
                    y.checked_sub(1).map(|n| (n, x)),
                    (x + 1 < w).then_some((y, x + 1)),
                    x.checked_sub(1).map(|n| (y, n)),
                ]
                .into_iter()
                .flatten()
                {
                    if ny >= h || nx >= w {
                        continue;
                    }
                    let at = ny * w + nx;
                    if grown[at] && !visited[at] {
                        visited[at] = true;
                        stack.push((ny, nx));
                    }
                }
            }
            if count > 40 {
                boxes.push((min_x, min_y, max_x + 1, max_y + 1));
            }
        }
    }
    if boxes.is_empty() {
        return Err(
            "No foreground drawings found. Check --bg-tol or image alpha.".into(),
        );
    }
    let largest = boxes
        .iter()
        .map(|b| (b.2 - b.0) * (b.3 - b.1))
        .max()
        .unwrap_or(0);
    boxes.retain(|b| (b.2 - b.0) * (b.3 - b.1) >= (0.14 * largest as f64) as usize);
    let band = (h as f64 / 2.2).max(1.0);
    boxes.sort_by_key(|b| (round_even((b.1 + b.3) as f64 / 2.0 / band), b.0));
    if boxes.len() != 4 {
        let list: Vec<String> = boxes
            .iter()
            .map(|b| format!("({}, {}, {}, {})", b.0, b.1, b.2 - b.0, b.3 - b.1))
            .collect();
        return Err(format!(
            "Found {} drawings [{}], expected 4 (top, side, front, back). Check spacing between views or adjust --bg-tol.",
            boxes.len(),
            list.join(", ")
        ));
    }
    let mut views: Vec<Plane> = boxes
        .iter()
        .map(|&(x0, y0, x1, y1)| trim_view(&im.crop(y0, x0, y1, x1)))
        .collect();
    if order == "reading" {
        return Ok(views.iter().map(Plane::json).collect());
    }
    reorder_views(&mut views);
    Ok(views.iter().map(Plane::json).collect())
}

// Pick TOP/SIDE by shape ratios; keep the first pairing within 10% of the best.
fn reorder_views(views: &mut Vec<Plane>) {
    if views.len() != 4 {
        return;
    }
    let mismatch = |a: &Plane, b: &Plane, others: &[Plane]| -> f64 {
        let mut value = (a.rows as f64 / b.cols as f64).abs().ln().abs();
        for other in others {
            value += ((a.cols as f64 / other.cols as f64).ln().abs()
                + (b.rows as f64 / other.rows as f64).ln().abs())
                / 2.0;
        }
        value
    };
    let mut pairs = Vec::new();
    for top in 0..4 {
        for side in 0..4 {
            if top == side {
                continue;
            }
            let others: Vec<Plane> = views
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != top && i != side)
                .map(|(_, v)| v.clone())
                .collect();
            let score = mismatch(&views[top], &views[side], &others);
            pairs.push((score, top, side));
        }
    }
    let best = pairs
        .iter()
        .map(|p| p.0)
        .fold(f64::INFINITY, f64::min);
    let (_, t, s) = pairs
        .into_iter()
        .find(|p| p.0 <= best + 0.1)
        .unwrap_or((0.0, 0, 1));
    let mut ordered = vec![views[t].clone(), views[s].clone()];
    for (i, v) in views.iter().enumerate() {
        if i != t && i != s {
            ordered.push(v.clone());
        }
    }
    *views = ordered;
}

pub fn find_views(
    views: Vec<serde_json::Value>,
    order: &str,
) -> Result<Vec<serde_json::Value>, String> {
    if views.len() == 1 {
        return sheet_views(&views[0], order);
    }
    let mut cells: Vec<(f64, usize, Plane)> = Vec::new();
    for (i, v) in views.iter().enumerate() {
        let im = plane_from(v).map_err(|_| index_error(v, "Ellipsis"))?;
        let band = (im.rows as f64 / 2.2).max(1.0);
        let centre = im.rows as f64 / 2.0 / band;
        cells.push((centre, i, trim_view(&im)));
    }
    cells.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut ordered: Vec<Plane> = cells.into_iter().map(|(_, _, p)| p).collect();
    if order != "reading" {
        reorder_views(&mut ordered);
    }
    Ok(ordered.iter().map(Plane::json).collect())
}

pub fn strip_outline(v: serde_json::Value, dark: i64) -> Result<serde_json::Value, String> {
    if dark <= 0 {
        return Ok(v);
    }
    let mut im = plane_from(&v).map_err(|_| index_error(&v, "Ellipsis"))?;
    let (h, w) = (im.rows, im.cols);
    let filled = |im: &Plane, y: usize, x: usize| im.alpha(y, x) > 0;
    let mut outline = vec![false; h * w];
    for y in 0..h {
        for x in 0..w {
            if !filled(&im, y, x) {
                continue;
            }
            let mut edge = false;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let ny = y as isize + dy;
                    let nx = x as isize + dx;
                    if ny < 0 || nx < 0 || ny >= h as isize || nx >= w as isize {
                        edge = true;
                    } else if !filled(&im, ny as usize, nx as usize) {
                        edge = true;
                    }
                }
            }
            if !edge {
                continue;
            }
            let luma = (0..3).map(|ch| f64::from(im.get(y, x, ch))).sum::<f64>() / 3.0;
            outline[y * w + x] = luma < dark as f64;
        }
    }
    const NEIGHBOURS: [(isize, isize); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    for y in 0..h {
        for x in 0..w {
            if !outline[y * w + x] {
                continue;
            }
            for &(dy, dx) in &NEIGHBOURS {
                let ny = y as isize + dy;
                let nx = x as isize + dx;
                if ny < 0 || nx < 0 || ny >= h as isize || nx >= w as isize {
                    continue;
                }
                let (ny, nx) = (ny as usize, nx as usize);
                if filled(&im, ny, nx) && !outline[ny * w + nx] {
                    for ch in 0..3 {
                        im.set(y, x, ch, im.get(ny, nx, ch));
                    }
                    break;
                }
            }
        }
    }
    Ok(im.json())
}

#[derive(Clone, Default)]
struct CutBox {
    colors: Vec<[u8; 3]>,
}

fn box_volume(cbox: &CutBox) -> (usize, usize, u32) {
    let spans = [0usize, 1, 2].map(|ch| {
        let low = cbox.colors.iter().map(|p| p[ch]).min().unwrap_or(0) as usize;
        let high = cbox.colors.iter().map(|p| p[ch]).max().unwrap_or(0) as usize;
        high - low + 1
    });
    (
        spans[0] * spans[1] * spans[2],
        cbox.colors.len(),
        (spans[0] as u32) * 256 + (spans[1] as u32) * 16 + (spans[2] as u32),
    )
}

// Weighted channel spans and a count-weighted median follow the upstream reference's cuts.
fn median_cut(colors: &[[u8; 3]], wanted: usize) -> Vec<[u8; 3]> {
    if colors.is_empty() {
        return Vec::new();
    }
    let mut groups = vec![CutBox { colors: colors.to_vec() }];
    while groups.len() < wanted {
        let mut pick = None;
        for (i, group) in groups.iter().enumerate() {
            if group.colors.len() < 2 {
                continue;
            }
            let (volume, count, span_key) = box_volume(group);
            if volume == 1 {
                continue;
            }
            let key = (count, volume, span_key);
            if pick.is_none_or(|(_, _, old): (usize, usize, (usize, usize, u32))| key > old) {
                pick = Some((i, key.0, key));
            }
        }
        let Some((index, _, _)) = pick else { break };
        let mut group = std::mem::take(&mut groups[index]).colors;
        let weights = [77i32, 150, 29];
        let widest = [0usize, 1, 2]
            .into_iter()
            .max_by_key(|&ch| {
                let low = group.iter().map(|p| p[ch] as i32).min().unwrap_or(0);
                let high = group.iter().map(|p| p[ch] as i32).max().unwrap_or(0);
                (high - low) * weights[ch]
            })
            .unwrap_or(0);
        group.sort_by_key(|p| p[widest]);
        let half = group.len() / 2;
        let right = group.split_off(half);
        groups[index] = CutBox { colors: group };
        groups.push(CutBox { colors: right });
    }
    groups
        .into_iter()
        .filter(|g| !g.colors.is_empty())
        .map(|g| {
            let count = g.colors.len() as u32;
            std::array::from_fn(|ch| {
                let sum = g.colors.iter().map(|p| u32::from(p[ch])).sum::<u32>();
                ((sum + count / 2) / count).min(255) as u8
            })
        })
        .collect()
}

pub fn one_palette(views: serde_json::Value, n: i64) -> Result<serde_json::Value, String> {
    if n <= 0 {
        return Ok(views);
    }
    let list = views.as_array().ok_or_else(|| not_iterable(&views))?;
    let mut images = Vec::with_capacity(list.len());
    for v in list {
        images.push(plane_from(v).map_err(|_| index_error(v, "Ellipsis"))?);
    }
    if images.is_empty() {
        return Err("need at least one array to concatenate".into());
    }
    let mut colours: Vec<[u8; 3]> = Vec::new();
    let mut seen = HashSet::new();
    for im in &images {
        for y in 0..im.rows {
            for x in 0..im.cols {
                if im.bands > 3 && im.alpha(y, x) == 0 {
                    continue;
                }
                let key = std::array::from_fn(|ch| (im.get(y, x, ch) / 6) * 6 + 3);
                if seen.insert(key) {
                    colours.push(key);
                }
            }
        }
    }
    if colours.is_empty() {
        return Err("need at least one array to concatenate".into());
    }
    colours.sort();
    let chosen = median_cut(&colours, (n as usize).min(colours.len()));
    let mut out = Vec::with_capacity(images.len());
    for im in &mut images {
        for y in 0..im.rows {
            for x in 0..im.cols {
                let mut closest = chosen[0];
                let mut shortest = u64::MAX;
                for &candidate in &chosen {
                    let distance: u64 = (0..3)
                        .map(|ch| {
                            let delta = i32::from(im.get(y, x, ch)) - i32::from(candidate[ch]);
                            (delta * delta) as u64
                        })
                        .sum();
                    if distance < shortest {
                        shortest = distance;
                        closest = candidate;
                    }
                }
                for ch in 0..3 {
                    im.set(y, x, ch, closest[ch]);
                }
            }
        }
        out.push(im.json());
    }
    Ok(Value::Array(out))
}

pub fn fit(v: serde_json::Value, w: i64, h: i64) -> Result<serde_json::Value, String> {
    let im = plane_from(&v).map_err(|_| interface_error(&v))?;
    Ok(resize(&im, w, h).json())
}

pub fn edge_colour(view: serde_json::Value, row: i64, i: i64) -> Result<serde_json::Value, String> {
    let im = plane_from(&view).map_err(|_| index_error(&view, &row.to_string()))?;
    if row < 0 || row as usize >= im.rows {
        return Ok(Value::Null);
    }
    let y = row as usize;
    let edges: Vec<usize> = (0..im.cols).filter(|&x| im.alpha(y, x) != 0).collect();
    if edges.is_empty() {
        return Ok(Value::Null);
    }
    let x = clamp_to(i, edges[0], *edges.last().unwrap());
    Ok(Value::Array(
        (0..3).map(|ch| Value::from(im.get(y, x, ch))).collect(),
    ))
}

pub fn match_top(top: serde_json::Value, side: serde_json::Value) -> Result<String, String> {
    let sv = plane_from(&side).map_err(|_| index_error(&side, "Ellipsis"))?;
    let tv = plane_from(&top).map_err(|_| index_error(&top, "Ellipsis"))?;
    let mut heights = vec![0.0; sv.cols];
    for y in 0..sv.rows {
        for x in 0..sv.cols {
            if sv.alpha(y, x) > 0 {
                heights[x] += 1.0;
            }
        }
    }
    let peak = heights.iter().copied().fold(0.0, f64::max);
    let height_at = |pos: f64| -> f64 {
        let p = pos.clamp(0.0, (heights.len() - 1) as f64);
        let low = p.floor() as usize;
        let high = (low + 1).min(heights.len() - 1);
        let fraction = p - low as f64;
        heights[low] * (1.0 - fraction) + heights[high] * fraction
    };
    let mut tall = vec![false; tv.rows];
    for i in 0..tv.rows {
        let pos = if tv.rows > 1 {
            i as f64 * (heights.len() - 1) as f64 / (tv.rows - 1) as f64
        } else {
            0.0
        };
        tall[i] = height_at(pos) >= 0.8 * peak;
    }
    let mut dark = vec![0.0; tv.rows];
    for y in 0..tv.rows {
        let mut count = 0.0;
        for x in 0..tv.cols {
            let rgb: i32 = (0..3).map(|ch| i32::from(tv.get(y, x, ch))).sum();
            if rgb < 240 && (tv.bands <= 3 || tv.alpha(y, x) > 0) {
                count += 1.0;
            }
        }
        dark[y] = count / tv.cols as f64;
    }
    let (mut forward, mut backward) = (0.0, 0.0);
    for i in 0..tv.rows {
        if tall[i] {
            forward += dark[i];
            backward += dark[tv.rows - 1 - i];
        }
    }
    Ok(if forward >= backward { "down" } else { "up" }.into())
}

fn edge_row(im: &Plane, row: i64, length: usize) -> Option<Vec<[u8; 3]>> {
    if row < 0 || row as usize >= im.rows {
        return None;
    }
    let y = row as usize;
    let edges: Vec<usize> = (0..im.cols).filter(|&x| im.alpha(y, x) > 0).collect();
    if edges.is_empty() {
        return None;
    }
    let first = edges[0];
    let last = *edges.last().unwrap();
    Some(
        (0..length)
            .map(|i| {
                let x = clamp_to(i as i64, first, last);
                [im.get(y, x, 0), im.get(y, x, 1), im.get(y, x, 2)]
            })
            .collect(),
    )
}

fn assemble(views: &[Plane; 4], shade: f64, round: bool) -> Block {
    let (top, side, front, back) = (&views[0], &views[1], &views[2], &views[3]);
    let (length, width, depth) = (top.rows, top.cols, side.rows);
    let mut occupied = vec![false; depth * length * width];
    for z in 0..depth {
        let row = depth - 1 - z;
        let base = z * length * width;
        let mut along = vec![false; length];
        let mut across = vec![false; width];
        for y in 0..length {
            along[y] = side.alpha(row, y.min(side.cols - 1)) > 0;
        }
        for x in 0..width {
            across[x] = front.alpha(row, x.min(front.cols - 1)) > 0
                || back.alpha(row, x.min(back.cols - 1)) > 0;
        }
        for y in 0..length {
            for x in 0..width {
                occupied[base + y * width + x] =
                    top.alpha(y, x.min(top.cols - 1)) > 0 && along[y] && across[x];
            }
        }
        if round {
            let ys: Vec<usize> = along.iter().enumerate().filter(|(_, v)| **v).map(|(i, _)| i).collect();
            let xs: Vec<usize> = across.iter().enumerate().filter(|(_, v)| **v).map(|(i, _)| i).collect();
            if !ys.is_empty() && !xs.is_empty() {
                let (y0, y1) = (ys[0], *ys.last().unwrap());
                let (x0, x1) = (xs[0], *xs.last().unwrap());
                let continuous = along[y0..=y1].iter().all(|v| *v)
                    && across[x0..=x1].iter().all(|v| *v);
                if continuous {
                    let (cy, cx) = ((y0 + y1) as f64 / 2.0, (x0 + x1) as f64 / 2.0);
                    let (ry, rx) = ((y1 - y0) as f64 / 2.0 + 0.5, (x1 - x0) as f64 / 2.0 + 0.5);
                    for y in 0..length {
                        for x in 0..width {
                            let dy = (y as f64 - cy) / ry;
                            let dx = (x as f64 - cx) / rx;
                            if dy * dy + dx * dx >= 1.0 {
                                occupied[base + y * width + x] = false;
                            }
                        }
                    }
                }
            }
        }
    }

    let mut volume = Block::new(depth, length, width);
    let mut above_mask = vec![false; length * width];
    for z in (0..depth).rev() {
        let row = depth - 1 - z;
        let above = above_mask.clone();
        let base = z * length * width;
        let slice = &occupied[base..base + length * width];
        for i in 0..above_mask.len() {
            above_mask[i] |= slice[i];
        }
        if !slice.iter().any(|v| *v) {
            continue;
        }

        let mut nearest: [Vec<usize>; 4] =
            std::array::from_fn(|_| vec![0usize; length * width]);
        for y in 0..length {
            let mut empty_left = -1i64;
            let mut empty_right = width as i64;
            for x in 0..width {
                let i = y * width + x;
                if !slice[i] {
                    empty_left = x as i64;
                }
                nearest[0][i] = (x as i64 - empty_left) as usize;
            }
            for x in (0..width).rev() {
                let i = y * width + x;
                if !slice[i] {
                    empty_right = x as i64;
                }
                nearest[1][i] = (empty_right - x as i64) as usize;
            }
        }
        for x in 0..width {
            let mut empty_up = -1i64;
            let mut empty_down = length as i64;
            for y in 0..length {
                let i = y * width + x;
                if !slice[i] {
                    empty_up = y as i64;
                }
                nearest[2][i] = (y as i64 - empty_up) as usize;
            }
            for y in (0..length).rev() {
                let i = y * width + x;
                if !slice[i] {
                    empty_down = y as i64;
                }
                nearest[3][i] = (empty_down - y as i64) as usize;
            }
        }

        let mut close_above = vec![false; length * width];
        for dy in 0usize..5 {
            for dx in 0usize..5 {
                for y in 0..length {
                    for x in 0..width {
                        let py = y as i64 + dy as i64 - 2;
                        let px = x as i64 + dx as i64 - 2;
                        if py >= 0 && py < length as i64 && px >= 0 && px < width as i64 {
                            close_above[y * width + x] |=
                                above[py as usize * width + px as usize];
                        }
                    }
                }
            }
        }

        let side_row = edge_row(side, row as i64, length);
        let back_row = edge_row(back, row as i64, width);
        let front_row = edge_row(front, row as i64, width);
        for y in 0..length {
            for x in 0..width {
                let i = y * width + x;
                if !slice[i] {
                    continue;
                }
                let mut face = 0;
                let mut depth_to_edge = usize::MAX;
                for f in 1..4 {
                    if nearest[f][i] < depth_to_edge {
                        depth_to_edge = nearest[f][i];
                        face = f;
                    }
                }
                if nearest[0][i] <= depth_to_edge {
                    depth_to_edge = nearest[0][i];
                    face = 0;
                }
                let steep = !above[i] && depth_to_edge <= 2 && close_above[i];
                let use_top = !above[i] && !steep;
                let use_above = above[i] && depth_to_edge > 2 && z + 1 < depth
                    && volume.at(z + 1, y, x)[3] > 0;
                let mut rgb = [top.get(y, x, 0), top.get(y, x, 1), top.get(y, x, 2)];
                if !use_top && !use_above {
                    let row_colour = match face {
                        0 | 1 => side_row.as_ref().map(|r| r[y]),
                        2 => back_row.as_ref().map(|r| r[x]),
                        _ => front_row.as_ref().map(|r| r[x]),
                    };
                    if let Some(c) = row_colour {
                        rgb = c;
                    }
                }
                if use_above {
                    let c = volume.at(z + 1, y, x);
                    rgb = [c[0], c[1], c[2]];
                }
                if shade != 0.0 && (above[i] || steep) && depth_to_edge <= 2 {
                    let factor = 1.0
                        - shade * (1.0 - z as f64 / (depth - 1).max(1) as f64)
                        - shade * 0.4;
                    for ch in 0..3 {
                        rgb[ch] = (f64::from(rgb[ch]) * factor).clamp(0.0, 255.0) as u8;
                    }
                }
                volume.put(z, y, x, [rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    volume
}

pub fn carve(
    top: serde_json::Value,
    side: serde_json::Value,
    front: serde_json::Value,
    back: serde_json::Value,
    slices: i64,
    round: bool,
) -> Result<serde_json::Value, String> {
    let mut views = [
        plane_from(&top)?,
        plane_from(&side)?,
        plane_from(&front)?,
        plane_from(&back)?,
    ];
    if slices > 0 {
        let depth = slices as usize;
        views[1] = resize(&views[1], views[1].cols as i64, depth as i64);
        views[2] = resize(&views[2], views[2].cols as i64, depth as i64);
        views[3] = resize(&views[3], views[3].cols as i64, depth as i64);
    }
    Ok(assemble(&views, 0.0, round).json())
}

pub fn views_to_slices(
    sheet: serde_json::Value,
    slices: Option<i64>,
    top_nose: String,
    side_nose: String,
    order: String,
    colours: i64,
    shade: f64,
    dark_thresh: i64,
    bg_tol: i64,
    round: bool,
) -> Result<serde_json::Value, String> {
    let clean = remove_background(sheet, bg_tol)?;
    let raw = sheet_views(&clean, &order)?;
    if raw.len() != 4 {
        return Err("expected 4 views".into());
    }
    let mut views: Vec<Plane> = raw
        .iter()
        .map(|v| plane_from(v).map_err(|_| index_error(v, "Ellipsis")))
        .collect::<Result<_, _>>()?;
    let facing = side_nose.clone();
    let mut side = if facing == "left" {
        views[1].flip_columns()
    } else {
        views[1].clone()
    };
    let raw_height = side.rows;
    let wanted = match slices {
        None => {
            if raw_height > 64 { 36 } else { raw_height }
        }
        Some(0) => raw_height,
        Some(n) => n.max(1) as usize,
    };
    let (length, width, depth);
    if wanted != raw_height {
        length = ratio_round(wanted * side.cols, side.rows).max(4);
        width = ratio_round(length * views[0].cols, views[0].rows).max(4);
        depth = wanted;
        views[0] = resize(&views[0], width as i64, length as i64);
        side = resize(&side, length as i64, depth as i64);
        views[2] = resize(&views[2], width as i64, depth as i64);
        views[3] = resize(&views[3], width as i64, depth as i64);
    } else {
        length = views[0].rows;
        width = views[0].cols;
        depth = ratio_round(raw_height * length, side.cols).max(2);
        side = resize(&side, length as i64, depth as i64);
        views[2] = resize(&views[2], width as i64, depth as i64);
        views[3] = resize(&views[3], width as i64, depth as i64);
    }
    views[1] = side;
    views[3] = views[3].flip_columns();

    let prepared: Vec<Value> = views
        .iter()
        .map(|v| strip_outline(v.json(), dark_thresh))
        .collect::<Result<_, _>>()?;
    let paletted = one_palette(Value::Array(prepared), colours)?;
    views = paletted
        .as_array()
        .ok_or_else(|| "bad palette".to_string())?
        .iter()
        .map(|v| plane_from(v).map_err(|_| index_error(v, "Ellipsis")))
        .collect::<Result<_, _>>()?;

    let mut nose = top_nose;
    if nose == "auto" {
        nose = match_top(views[0].json(), views[1].json())?;
    }
    if nose == "up" {
        views[0] = views[0].flip_rows();
    }

    let display = vec![
        views[0].json(),
        if facing == "left" {
            views[1].flip_columns().json()
        } else {
            views[1].json()
        },
        views[2].json(),
        views[3].flip_columns().json(),
    ];
    let carved: [Plane; 4] = std::array::from_fn(|i| views[i].clone());
    Ok(Value::Array(vec![assemble(&carved, shade, round).json(), Value::Array(display)]))
}

fn paint_stack(
    vol: &Block,
    angle_deg: f64,
    squash: f64,
    spacing: f64,
    shadow: bool,
    outline: bool,
) -> Plane {
    let (n, h, w) = (vol.depth, vol.rows, vol.cols);
    let diag = (w as f64).hypot(h as f64).ceil() as usize + 8;
    let lift = ((n.saturating_sub(1)) as f64 * spacing).ceil() as usize;
    let canvas_w = diag + 12;
    let canvas_h = (diag as f64 * squash).ceil() as usize + lift + 16;
    let centre_x = (canvas_w / 2) as i64;
    let base_y = canvas_h as i64 - (diag as f64 * squash / 2.0).ceil() as i64 - 6;
    let radians = angle_deg.to_radians();
    let (cosine, sine) = (radians.cos(), radians.sin());
    let mut canvas = Plane::new(canvas_h, canvas_w, 4);

    if shadow && squash != 0.0 {
        let mut footprint = vec![false; h * w];
        for z in 0..n {
            for y in 0..h {
                for x in 0..w {
                    if vol.at(z, y, x)[3] > 0 {
                        footprint[y * w + x] = true;
                    }
                }
            }
        }
        for py in 0..canvas_h {
            for px in 0..canvas_w {
                let dx = px as i64 - centre_x - 2;
                let dy = (py as i64 - base_y - 3) as f64 / squash;
                let sx = (dx as f64 * cosine + dy * sine + w as f64 / 2.0).floor() as i64;
                let sy = (-dx as f64 * sine + dy * cosine + h as f64 / 2.0).floor() as i64;
                if sx >= 0 && sx < w as i64 && sy >= 0 && sy < h as i64
                    && footprint[sy as usize * w + sx as usize]
                {
                    canvas.paint(py, px, [186, 198, 212, 255]);
                }
            }
        }
    }

    let mut body = Plane::new(canvas_h, canvas_w, 4);
    if squash != 0.0 {
        for z in 0..n {
            let rise = round_even(z as f64 * spacing);
            for py in 0..canvas_h {
                for px in 0..canvas_w {
                    let dx = px as i64 - centre_x;
                    let dy = (py as i64 - (base_y - rise)) as f64 / squash;
                    let sx = (dx as f64 * cosine + dy * sine + w as f64 / 2.0).floor() as i64;
                    let sy = (-dx as f64 * sine + dy * cosine + h as f64 / 2.0).floor() as i64;
                    if sx >= 0 && sx < w as i64 && sy >= 0 && sy < h as i64 {
                        let p = vol.at(z, sy as usize, sx as usize);
                        if p[3] > 0 {
                            body.paint(py, px, p);
                        }
                    }
                }
            }
        }
    }

    if outline {
        for py in 0..canvas_h {
            for px in 0..canvas_w {
                if body.alpha(py, px) > 0 {
                    continue;
                }
                let adjacent = [
                    py.checked_sub(1).map(|y| (y, px)),
                    (py + 1 < canvas_h).then_some((py + 1, px)),
                    px.checked_sub(1).map(|x| (py, x)),
                    (px + 1 < canvas_w).then_some((py, px + 1)),
                ];
                if adjacent.into_iter().flatten().any(|(y, x)| body.alpha(y, x) > 0) {
                    canvas.paint(py, px, [24, 28, 36, 255]);
                }
            }
        }
    }
    for py in 0..canvas_h {
        for px in 0..canvas_w {
            if body.alpha(py, px) > 0 {
                canvas.paint(py, px, body.rgba(py, px));
            }
        }
    }
    canvas
}

pub fn render_stack_image(
    vol: serde_json::Value,
    angle: f64,
    squash: f64,
    spacing: f64,
    shadow: bool,
    outline: bool,
) -> Result<serde_json::Value, String> {
    let block = block_from(&vol)?;
    Ok(paint_stack(&block, angle, squash, spacing, shadow, outline).json())
}

pub fn thin_runs(vol: serde_json::Value, min_run: i64) -> Result<Vec<serde_json::Value>, String> {
    let block = block_from(&vol).map_err(|_| index_error(&vol, "Ellipsis"))?;
    let mut thin = Vec::with_capacity(block.depth);
    for z in 0..block.depth {
        let mut solid = vec![false; block.rows * block.cols];
        let mut sturdy = vec![false; solid.len()];
        for y in 0..block.rows {
            for x in 0..block.cols {
                solid[y * block.cols + x] = block.at(z, y, x)[3] > 0;
            }
        }
        for y in 0..block.rows.saturating_sub(1) {
            for x in 0..block.cols.saturating_sub(1) {
                let at = y * block.cols + x;
                if solid[at] && solid[at + block.cols] && solid[at + 1]
                    && solid[at + block.cols + 1]
                {
                    sturdy[at] = true;
                    sturdy[at + 1] = true;
                    sturdy[at + block.cols] = true;
                    sturdy[at + block.cols + 1] = true;
                }
            }
        }
        let mut seen = vec![false; solid.len()];
        let mut fragile = false;
        for y in 0..block.rows {
            for x in 0..block.cols {
                let at = y * block.cols + x;
                if !solid[at] || seen[at] || fragile {
                    continue;
                }
                seen[at] = true;
                let mut stack = vec![(y, x)];
                let mut has_block = false;
                while let Some((cy, cx)) = stack.pop() {
                    has_block |= sturdy[cy * block.cols + cx];
                    for (ny, nx) in [
                        (cy + 1 < block.rows).then_some((cy + 1, cx)),
                        cy.checked_sub(1).map(|n| (n, cx)),
                        (cx + 1 < block.cols).then_some((cy, cx + 1)),
                        cx.checked_sub(1).map(|n| (cy, n)),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if ny >= block.rows || nx >= block.cols {
                            continue;
                        }
                        let next = ny * block.cols + nx;
                        if solid[next] && !seen[next] {
                            seen[next] = true;
                            stack.push((ny, nx));
                        }
                    }
                }
                fragile = !has_block;
            }
        }
        thin.push(fragile);
    }
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for z in 0..=thin.len() {
        let fragile = z < thin.len() && thin[z];
        if fragile {
            if start.is_none() {
                start = Some(z);
            }
        } else if let Some(first) = start.take() {
            if (z - first) as i64 >= min_run {
                runs.push(Value::Array(vec![
                    Value::from(first as i64),
                    Value::from((z - 1) as i64),
                ]));
            }
        }
    }
    Ok(runs)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut check = Vec::with_capacity(4 + data.len());
    check.extend_from_slice(kind);
    check.extend_from_slice(data);
    out.extend_from_slice(&crc32(&check).to_be_bytes());
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (a, b, c) = (i32::from(a), i32::from(b), i32::from(c));
    let guess = a + b - c;
    let (da, db, dc) = ((guess - a).abs(), (guess - b).abs(), (guess - c).abs());
    if da <= db && da <= dc {
        a as u8
    } else if db <= dc {
        b as u8
    } else {
        c as u8
    }
}

fn encode_png(im: &Plane) -> Vec<u8> {
    let stride = im.cols * im.bands;
    let mut raw = Vec::with_capacity(im.rows * (stride + 1));
    let mut previous = vec![0u8; stride];
    for y in 0..im.rows {
        let line: Vec<u8> = (0..im.cols)
            .flat_map(|x| (0..im.bands).map(move |ch| im.get(y, x, ch)))
            .collect();
        let mut winner = (0u8, line.clone(), u64::MAX);
        for filter in [0u8, 1, 2, 4] {
            let mut filtered = Vec::with_capacity(stride);
            for i in 0..stride {
                let a = i.checked_sub(im.bands).map_or(0, |j| line[j]);
                let b = previous[i];
                let c = i.checked_sub(im.bands).map_or(0, |j| previous[j]);
                let predictor = match filter {
                    0 => 0,
                    1 => a,
                    2 => b,
                    _ => paeth(a, b, c),
                };
                filtered.push(line[i].wrapping_sub(predictor));
            }
            let score: u64 = filtered
                .iter()
                .map(|&v| u64::from(u32::from(v).min(256 - u32::from(v))))
                .sum();
            if score < winner.2 {
                winner = (filter, filtered, score);
            }
        }
        raw.push(winner.0);
        raw.extend_from_slice(&winner.1);
        previous = line;
    }

    let config = zlib_rs::DeflateConfig {
        level: -1,
        mem_level: 8,
        strategy: zlib_rs::Strategy::Filtered,
        ..Default::default()
    };
    let mut buffer = vec![0u8; zlib_rs::compress_bound(raw.len())];
    let (compressed, _) = zlib_rs::compress_slice(&mut buffer, &raw, config);
    let idat = compressed.to_vec();

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&(im.cols as u32).to_be_bytes());
    header.extend_from_slice(&(im.rows as u32).to_be_bytes());
    header.extend_from_slice(&[8, if im.bands == 3 { 2 } else { 6 }, 0, 0, 0]);
    png_chunk(&mut out, b"IHDR", &header);
    png_chunk(&mut out, b"IDAT", &idat);
    png_chunk(&mut out, b"IEND", &[]);
    out
}

fn from_pixels<const N: usize>(cols: usize, pixels: &[[u8; N]]) -> Plane {
    if cols == 0 {
        return Plane::new(0, 0, N);
    }
    let rows = pixels.len() / cols;
    let mut im = Plane::new(rows, cols, N);
    for (i, pixel) in pixels.iter().enumerate().take(rows * cols) {
        for ch in 0..N {
            im.data[i * N + ch] = pixel[ch];
        }
    }
    im
}

pub fn png_bytes(w: usize, h: usize, pixels: &[[u8; 4]]) -> Vec<u8> {
    let mut im = from_pixels(w, pixels);
    if im.rows > h {
        im = im.crop(0, 0, h, im.cols);
    }
    encode_png(&im)
}

pub fn png_bytes_rgb(w: usize, h: usize, pixels: &[[u8; 3]]) -> Vec<u8> {
    let mut im = from_pixels(w, pixels);
    if im.rows > h {
        im = im.crop(0, 0, h, im.cols);
    }
    encode_png(&im)
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for triple in bytes.chunks(3) {
        let b = [
            triple[0],
            triple.get(1).copied().unwrap_or(0),
            triple.get(2).copied().unwrap_or(0),
        ];
        let bits = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(bits >> 18) as usize] as char);
        out.push(ALPHABET[((bits >> 12) & 63) as usize] as char);
        out.push(if triple.len() > 1 {
            ALPHABET[((bits >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if triple.len() > 2 {
            ALPHABET[(bits & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

pub fn to_data_uri(arr: serde_json::Value) -> Result<String, String> {
    let im = plane_from(&arr).map_err(|_| interface_error(&arr))?;
    Ok(format!("data:image/png;base64,{}", base64(&encode_png(&im))))
}

fn draw_label(im: &mut Plane, x0: usize, y0: usize, text: &str, color: [u8; 4]) {
    let mut pen = 0i32;
    for ch in text.chars() {
        let Some(glyph) = crate::glyphs::glyph(ch as usize - 32) else {
            continue;
        };
        for row in 0..glyph.h {
            for col in 0..glyph.w {
                let mask = u16::from(glyph.bits[row * glyph.w + col]);
                if mask == 0 {
                    continue;
                }
                let px = x0 as i32 + pen + glyph.dx + col as i32;
                let py = y0 as i32 + glyph.dy + row as i32;
                if px < 0 || py < 0 || px as usize >= im.cols || py as usize >= im.rows {
                    continue;
                }
                let (px, py) = (px as usize, py as usize);
                let under = im.rgba(py, px);
                let blend = |base: u8, ink: u8| {
                    ((u16::from(base) * (255 - mask) + u16::from(ink) * mask + 127) / 255) as u8
                };
                im.paint(
                    py,
                    px,
                    [
                        blend(under[0], color[0]),
                        blend(under[1], color[1]),
                        blend(under[2], color[2]),
                        255,
                    ],
                );
            }
        }
        pen += glyph.advance;
    }
}

pub fn build_preview(
    vol: serde_json::Value,
    views: serde_json::Value,
    out_path: serde_json::Value,
) -> Result<(), String> {
    let block = block_from(&vol)?;
    let mut renders = Vec::new();
    for (angle, gap) in [(35.0, 1.0), (135.0, 1.0), (225.0, 1.0), (315.0, 1.0), (35.0, 2.4)] {
        renders.push(paint_stack(&block, angle, 0.75, gap, true, true).json());
    }
    let pictures: Vec<Plane> = renders
        .iter()
        .map(|r| plane_from(r).map_err(|_| shape_error(r)))
        .collect::<Result<_, _>>()?;
    let extracted: Vec<Plane> = view_list(&views)?
        .iter()
        .map(|v| plane_from(v).map_err(|_| index_error(v, "Ellipsis")))
        .collect::<Result<_, _>>()?;

    let scale = round_even(180.0 / pictures[0].cols.max(1) as f64).clamp(2, 6) as usize;
    let pad = 16usize;
    let cell_w = pictures.iter().map(|im| im.cols * scale).max().unwrap_or(0) + pad;
    let cell_h = pictures.iter().map(|im| im.rows * scale).max().unwrap_or(0) + pad + 20;
    let top_h = extracted.iter().map(|im| im.rows * scale).max().unwrap_or(0) + 36;
    let total_w = (cell_w * 5 + pad).max(
        extracted
            .iter()
            .map(|im| im.cols * scale + pad * 2)
            .sum::<usize>(),
    );
    let total_h = top_h + cell_h + pad * 2;
    let mut sheet = Plane::new(total_h, total_w, 4);
    for y in 0..total_h {
        for x in 0..total_w {
            sheet.paint(y, x, [242, 245, 249, 255]);
        }
    }

    let mut x = pad;
    for (i, im) in extracted.iter().enumerate() {
        let label = ["TOP", "SIDE", "FRONT", "BACK"].get(i).copied().unwrap_or("");
        draw_label(
            &mut sheet,
            x,
            pad,
            &format!("{label} ({}x{})", im.cols, im.rows),
            [70, 80, 95, 255],
        );
        let enlarged = resize(im, (im.cols * scale) as i64, (im.rows * scale) as i64);
        for y in 0..enlarged.rows {
            for xx in 0..enlarged.cols {
                let py = pad + 18 + y;
                let px = x + xx;
                if enlarged.alpha(y, xx) > 0 && py < sheet.rows && px < sheet.cols {
                    sheet.paint(py, px, enlarged.rgba(y, xx));
                }
            }
        }
        x += enlarged.cols + pad * 2;
    }

    let row_top = top_h + pad;
    for (i, im) in pictures.iter().enumerate() {
        let label = ["35 deg", "135 deg", "225 deg", "315 deg", "exploded"]
            .get(i)
            .copied()
            .unwrap_or("");
        draw_label(
            &mut sheet,
            pad + i * cell_w + 4,
            row_top,
            label,
            [70, 80, 95, 255],
        );
        let enlarged = resize(im, (im.cols * scale) as i64, (im.rows * scale) as i64);
        let px = pad + i * cell_w + (cell_w - enlarged.cols) / 2;
        let py = row_top + 18 + (cell_h - 20 - enlarged.rows) / 2;
        for y in 0..enlarged.rows {
            for xx in 0..enlarged.cols {
                if enlarged.alpha(y, xx) > 0
                    && py + y < sheet.rows
                    && px + xx < sheet.cols
                {
                    sheet.paint(py + y, px + xx, enlarged.rgba(y, xx));
                }
            }
        }
    }

    let path = out_path
        .as_str()
        .ok_or_else(|| format!("expected str, got {}", type_name(&out_path)))?;
    let mut rgb = Vec::with_capacity(sheet.rows * sheet.cols);
    for y in 0..sheet.rows {
        for x in 0..sheet.cols {
            let p = sheet.rgba(y, x);
            rgb.push([p[0], p[1], p[2]]);
        }
    }
    std::fs::write(path, png_bytes_rgb(sheet.cols, sheet.rows, &rgb))
        .map_err(|e| format!("{e}"))
}

pub fn compute_normals(vol: serde_json::Value) -> Result<serde_json::Value, String> {
    let v = block_from(&vol).map_err(|_| index_error(&vol, "Ellipsis"))?;
    let (d, h, w) = (v.depth, v.rows, v.cols);
    let pw = w + 2;
    let ph = h + 2;
    let mut padded = vec![0.0f64; (d + 2) * ph * pw];
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if v.at(z, y, x)[3] > 0 {
                    padded[((z + 1) * ph + y + 1) * pw + x + 1] = 1.0;
                }
            }
        }
    }
    let at = |buf: &[f64], z: usize, y: usize, x: usize| buf[(z * ph + y) * pw + x];
    let triple = |a: f64, b: f64, c: f64| a + 2.0 * b + c;

    // Separable [1,2,1] smoothing along y then x, as in the upstream reference.
    let mut diff_z = vec![0.0f64; d * ph * pw];
    for z in 0..d {
        for y in 0..ph {
            for x in 0..pw {
                diff_z[(z * ph + y) * pw + x] =
                    at(&padded, z, y, x) - at(&padded, z + 2, y, x);
            }
        }
    }
    let mut nz = vec![0.0f64; d * h * w];
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                nz[(z * h + y) * w + x] = triple(
                    triple(
                        diff_z[(z * ph + y) * pw + x],
                        diff_z[(z * ph + y + 1) * pw + x],
                        diff_z[(z * ph + y + 2) * pw + x],
                    ),
                    triple(
                        diff_z[(z * ph + y) * pw + x + 1],
                        diff_z[(z * ph + y + 1) * pw + x + 1],
                        diff_z[(z * ph + y + 2) * pw + x + 1],
                    ),
                    triple(
                        diff_z[(z * ph + y) * pw + x + 2],
                        diff_z[(z * ph + y + 1) * pw + x + 2],
                        diff_z[(z * ph + y + 2) * pw + x + 2],
                    ),
                );
            }
        }
    }

    // Separable [1,2,1] smoothing along z then x.
    let mut diff_y = vec![0.0f64; (d + 2) * h * pw];
    for z in 0..(d + 2) {
        for y in 0..h {
            for x in 0..pw {
                diff_y[(z * h + y) * pw + x] =
                    at(&padded, z, y, x) - at(&padded, z, y + 2, x);
            }
        }
    }
    let mut ny = vec![0.0f64; d * h * w];
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let smooth_z = |x: usize| {
                    triple(
                        diff_y[(z * h + y) * pw + x],
                        diff_y[((z + 1) * h + y) * pw + x],
                        diff_y[((z + 2) * h + y) * pw + x],
                    )
                };
                ny[(z * h + y) * w + x] =
                    triple(smooth_z(x), smooth_z(x + 1), smooth_z(x + 2));
            }
        }
    }

    // Separable [1,2,1] smoothing along z then y.
    let mut diff_x = vec![0.0f64; (d + 2) * ph * w];
    for z in 0..(d + 2) {
        for y in 0..ph {
            for x in 0..w {
                diff_x[(z * ph + y) * w + x] =
                    at(&padded, z, y, x) - at(&padded, z, y, x + 2);
            }
        }
    }
    let mut nx = vec![0.0f64; d * h * w];
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let smooth_z = |y: usize| {
                    triple(
                        diff_x[(z * ph + y) * w + x],
                        diff_x[((z + 1) * ph + y) * w + x],
                        diff_x[((z + 2) * ph + y) * w + x],
                    )
                };
                nx[(z * h + y) * w + x] =
                    triple(smooth_z(y), smooth_z(y + 1), smooth_z(y + 2));
            }
        }
    }

    let mut out = Block::new(d, h, w);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if v.at(z, y, x)[3] == 0 {
                    continue;
                }
                let at3 = |buf: &[f64]| buf[(z * h + y) * w + x];
                let (gx, gy, mut gz) = (at3(&nx), at3(&ny), at3(&nz));
                let mut mag = (gx * gx + gy * gy + gz * gz).sqrt();
                if mag < 1e-5 {
                    gz = 1.0;
                    mag = 1.0;
                }
                let encode = |n: f64| {
                    ((n / mag * 0.5 + 0.5) * 255.0)
                        .round_ties_even()
                        .clamp(0.0, 255.0) as u8
                };
                out.put(z, y, x, [encode(gx), encode(gy), encode(gz), 255]);
            }
        }
    }
    Ok(out.json())
}

pub fn detect_emissive(vol: serde_json::Value) -> Result<serde_json::Value, String> {
    let v = block_from(&vol).map_err(|_| index_error(&vol, "Ellipsis"))?;
    let mut out = Block::new(v.depth, v.rows, v.cols);
    let solid = |z: i64, y: i64, x: i64| -> bool {
        if z < 0 || y < 0 || x < 0 || z >= v.depth as i64 || y >= v.rows as i64
            || x >= v.cols as i64
        {
            return false;
        }
        v.at(z as usize, y as usize, x as usize)[3] > 0
    };
    let mut surface = 0usize;
    let mut counts: HashMap<u32, usize> = HashMap::new();
    let mut candidates = Vec::new();
    for z in 0..v.depth {
        for y in 0..v.rows {
            for x in 0..v.cols {
                let p = v.at(z, y, x);
                if p[3] == 0 {
                    continue;
                }
                let zz = z as i64;
                let yy = y as i64;
                let xx = x as i64;
                let buried = solid(zz - 1, yy, xx)
                    && solid(zz + 1, yy, xx)
                    && solid(zz, yy - 1, xx)
                    && solid(zz, yy + 1, xx)
                    && solid(zz, yy, xx - 1)
                    && solid(zz, yy, xx + 1);
                if buried {
                    continue;
                }
                surface += 1;
                let (r, g, b) = (f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
                let luma = 0.299 * r + 0.587 * g + 0.114 * b;
                let high = r.max(g).max(b);
                let low = r.min(g).min(b);
                let saturation = (high - low) / high.max(1.0);
                let bright = (luma >= 180.0 && (saturation >= 0.30 || luma >= 232.0))
                    || (high >= 215.0 && saturation >= 0.62 && luma >= 110.0);
                let key = (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2]);
                *counts.entry(key).or_insert(0) += 1;
                if bright {
                    candidates.push((z, y, x, key));
                }
            }
        }
    }
    if surface == 0 {
        return Ok(out.json());
    }
    for (z, y, x, key) in candidates {
        let share = counts.get(&key).copied().unwrap_or(0) as f64 / surface as f64;
        if share <= 0.14 {
            out.put(z, y, x, v.at(z, y, x));
        }
    }
    Ok(out.json())
}

pub fn build_html_viewer(
    vol: serde_json::Value,
    views: serde_json::Value,
    out_path: serde_json::Value,
    title: String,
) -> Result<(), String> {
    let v = block_from(&vol).map_err(|_| "invalid volume".to_string())?;
    let mut sheet = Plane::new(v.rows, v.cols * v.depth, 4);
    for z in 0..v.depth {
        for y in 0..v.rows {
            for x in 0..v.cols {
                sheet.paint(y, z * v.cols + x, v.at(z, y, x));
            }
        }
    }
    let html = include_str!("viewer.html");
    let marker = "/*__EMBEDDED_STACK__*/null";
    if !html.contains(marker) {
        return Err("viewer template marker not found".into());
    }
    let supplied = view_list(&views)?;
    let view = |i: usize| -> Result<Value, String> {
        supplied
            .get(i)
            .cloned()
            .ok_or_else(|| format!("missing view {i}"))
    };
    let payload = serde_json::json!({
        "title": title,
        "slices": v.depth,
        "sliceWidth": v.cols,
        "sliceHeight": v.rows,
        "sheet": to_data_uri(sheet.json())?,
        "top": to_data_uri(view(0)?)?,
        "side": to_data_uri(view(1)?)?,
        "front": to_data_uri(view(2)?)?,
        "back": to_data_uri(view(3)?)?,
    });
    let path = out_path
        .as_str()
        .ok_or_else(|| format!("expected str, got {}", type_name(&out_path)))?;
    let embedded = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    std::fs::write(path, html.replacen(marker, &embedded, 1))
        .map_err(|e| format!("{e}"))
}
