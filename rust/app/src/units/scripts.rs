//! Sheet reader and engine front end: flags in, image strips and viewers out.

use std::ffi::OsString;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::ArgMatches;
use png::{ColorType, Decoder, Transformations};
use serde_json::{Value, json};

const NAME: &str = "views_to_slices";

const USAGE: &str = "usage: views_to_slices [-h] [--walsy-slices WALSY_SLICES]\n                        [--walsy-top-nose {auto,up,down}]\n                        [--walsy-side-nose {left,right}]\n                        [--walsy-order {auto,reading}]\n                        [--walsy-colours WALSY_COLOURS]\n                        [--walsy-compose {apple,banana,orange}]\n                        [--walsy-shade WALSY_SHADE] [--walsy-round]\n                        [--walsy-dark-thresh WALSY_DARK_THRESH]\n                        [--walsy-bg-tol WALSY_BG_TOL]\n                        [--walsy-preview WALSY_PREVIEW]\n                        [--walsy-html WALSY_HTML]\n                        [--walsy-normals WALSY_NORMALS]\n                        [--walsy-emissive WALSY_EMISSIVE]\n                        sheet out\n";

const HELP: &str = "usage: views_to_slices [-h] [--walsy-slices WALSY_SLICES]\n                        [--walsy-top-nose {auto,up,down}]\n                        [--walsy-side-nose {left,right}]\n                        [--walsy-order {auto,reading}]\n                        [--walsy-colours WALSY_COLOURS]\n                        [--walsy-compose {apple,banana,orange}]\n                        [--walsy-shade WALSY_SHADE] [--walsy-round]\n                        [--walsy-dark-thresh WALSY_DARK_THRESH]\n                        [--walsy-bg-tol WALSY_BG_TOL]\n                        [--walsy-preview WALSY_PREVIEW]\n                        [--walsy-html WALSY_HTML]\n                        [--walsy-normals WALSY_NORMALS]\n                        [--walsy-emissive WALSY_EMISSIVE]\n                        sheet out\n\nCarve four orthographic views into a horizontal slice strip.\n\npositional arguments:\n  sheet                 four-view sheet or an assembled four-view image\n  out                   output slice strip PNG (ground slice on the left)\n\noptions:\n  -h, --help            show this help message and exit\n  --walsy-slices WALSY_SLICES\n                        slice count; 0 keeps native slices (default: auto)\n  --walsy-top-nose {auto,up,down}\n                        which end of the top view is the nose (default: auto)\n  --walsy-side-nose {left,right}\n                        which end of the side view is the nose (default: left)\n  --walsy-order {auto,reading}\n                        view order in the sheet (default: auto)\n  --walsy-colours WALSY_COLOURS\n                        palette size (default: 24)\n  --walsy-compose {apple,banana,orange}\n                        generate the named four-view sheet before carving\n  --walsy-shade WALSY_SHADE\n                        depth shading amount (default: 0.2)\n  --walsy-round          round slice ends\n  --walsy-dark-thresh WALSY_DARK_THRESH\n                        dark outline threshold (default: 70)\n  --walsy-bg-tol WALSY_BG_TOL\n                        background tolerance (default: 28)\n  --walsy-preview WALSY_PREVIEW\n                        write an isometric preview PNG\n  --walsy-html WALSY_HTML\n                        write an interactive viewer\n  --walsy-normals WALSY_NORMALS\n                        write a normal-map PNG\n  --walsy-emissive WALSY_EMISSIVE\n                        write an emissive-mask PNG\n";

const MAX_DIM: usize = 10_000;
// shortcut: sheet resizing is O(slices^2); 128 covers documented dense strips, add a faster engine before raising.
const MAX_SLICES: i64 = 128;

// Long option names; a unique prefix is accepted, as with argparse.
const OPTIONS: &[(&str, &str)] = &[
    ("--help", ""),
    ("--walsy-slices", "WALSY_SLICES"),
    ("--walsy-top-nose", "{auto,up,down}"),
    ("--walsy-side-nose", "{left,right}"),
    ("--walsy-order", "{auto,reading}"),
    ("--walsy-colours", "WALSY_COLOURS"),
    ("--walsy-compose", "{apple,banana,orange}"),
    ("--walsy-shade", "WALSY_SHADE"),
    ("--walsy-round", ""),
    ("--walsy-dark-thresh", "WALSY_DARK_THRESH"),
    ("--walsy-bg-tol", "WALSY_BG_TOL"),
    ("--walsy-preview", "WALSY_PREVIEW"),
    ("--walsy-html", "WALSY_HTML"),
    ("--walsy-normals", "WALSY_NORMALS"),
    ("--walsy-emissive", "WALSY_EMISSIVE"),
];

#[derive(Default)]
struct Options {
    sheet: String,
    out: String,
    compose: Option<String>,
    explicit: Vec<&'static str>,
    slices: Option<i64>,
    top_nose: String,
    side_nose: String,
    order: String,
    colours: i64,
    shade: f64,
    rounded: bool,
    dark_thresh: i64,
    bg_tol: i64,
    preview: Option<String>,
    html: Option<String>,
    normals: Option<String>,
    emissive: Option<String>,
}

fn fail(message: &str) -> ExitCode {
    eprint!("{USAGE}");
    eprintln!("{NAME}: error: {message}");
    ExitCode::from(2)
}

fn one_of(flag: &str, value: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&value) {
        return Ok(());
    }
    let list = allowed
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "argument {flag}: invalid choice: '{value}' (choose from {list})"
    ))
}

fn resolve_flag(word: &str) -> Result<&'static str, String> {
    // A bare `--sli` is shorthand for `--walsy-slices`; `--he` means `--help`.
    if let Some(&(name, _)) = OPTIONS.iter().find(|(name, _)| *name == word) {
        return Ok(name);
    }
    let short = word.strip_prefix("--").unwrap_or("");
    if short.is_empty() {
        return Err(format!("unrecognized arguments: {word}"));
    }
    let expanded = format!("--walsy-{short}");
    let matches: Vec<&str> = OPTIONS
        .iter()
        .filter(|(name, _)| name.starts_with(word) || name.starts_with(&expanded))
        .map(|(name, _)| *name)
        .collect();
    if matches.is_empty() {
        return Err(format!("unrecognized arguments: {word}"));
    }
    match matches.as_slice() {
        [] => Err(format!("ambiguous option: {word} could match --walsy-*")),
        [only] => Ok(only),
        _ => Err(format!(
            "ambiguous option: {word} could match {}",
            matches.join(", ")
        )),
    }
}

// None means help was requested.
fn parse(raw: &[&OsString]) -> Result<Option<Options>, String> {
    let mut o = Options {
        top_nose: "auto".into(),
        side_nose: "left".into(),
        order: "auto".into(),
        colours: 24,
        shade: 0.2,
        dark_thresh: 70,
        bg_tol: 28,
        ..Options::default()
    };
    let mut positional = Vec::new();
    let mut literal = false;
    let mut i = 0;
    while i < raw.len() {
        let word = raw[i].to_string_lossy().into_owned();
        if literal {
            positional.push(word);
            i += 1;
            continue;
        }
        if word == "--" {
            literal = true;
            i += 1;
            continue;
        }
        if word == "-h" {
            return Ok(None);
        }
        if !word.starts_with("--") {
            positional.push(word);
            i += 1;
            continue;
        }
        let (spelling, inline) = match word.split_once('=') {
            Some((a, b)) => (a, Some(b.to_owned())),
            None => (word.as_str(), None),
        };
        let flag = resolve_flag(spelling)?;
        o.explicit.push(flag);
        if flag == "--help" {
            if inline.is_some() {
                return Err(format!("argument {flag}: ignored explicit argument"));
            }
            return Ok(None);
        }
        let value = if OPTIONS.iter().find(|(name, _)| *name == flag).unwrap().1.is_empty() {
            if inline.is_some() {
                return Err(format!(
                    "argument {flag}: ignored explicit argument '{}'",
                    inline.unwrap_or_default()
                ));
            }
            if flag == "--walsy-round" {
                o.rounded = true;
            }
            i += 1;
            continue;
        } else {
            match inline {
                Some(v) => v,
                None => {
                    i += 1;
                    raw.get(i)
                        .map(|v| v.to_string_lossy().into_owned())
                        .ok_or_else(|| format!("argument {flag}: expected one argument"))?
                }
            }
        };
        match flag {
            "--walsy-compose" => {
                one_of(flag, &value, scripts_rs::compose::PRESETS)?;
                o.compose = Some(value);
            }
            "--walsy-slices" => {
                let n: i64 = value
                    .parse()
                    .map_err(|_| format!("argument {flag}: invalid int value: '{value}'"))?;
                if n > MAX_SLICES {
                    return Err(format!(
                        "argument {flag}: out of range: '{value}' (max {MAX_SLICES})"
                    ));
                }
                usize::try_from(n)
                    .map_err(|_| format!("argument {flag}: invalid int value: '{value}'"))?;
                o.slices = Some(n);
            }
            "--walsy-top-nose" => {
                one_of(flag, &value, &["auto", "up", "down"])?;
                o.top_nose = value;
            }
            "--walsy-side-nose" => {
                one_of(flag, &value, &["left", "right"])?;
                o.side_nose = value;
            }
            "--walsy-order" => {
                one_of(flag, &value, &["auto", "reading"])?;
                o.order = value;
            }
            "--walsy-colours" => {
                o.colours = value
                    .parse()
                    .map_err(|_| format!("argument {flag}: invalid int value: '{value}'"))?;
            }
            "--walsy-shade" => {
                o.shade = value
                    .parse()
                    .map_err(|_| format!("argument {flag}: invalid float value: '{value}'"))?;
            }
            "--walsy-dark-thresh" => {
                o.dark_thresh = value
                    .parse()
                    .map_err(|_| format!("argument {flag}: invalid int value: '{value}'"))?;
            }
            "--walsy-bg-tol" => {
                o.bg_tol = value
                    .parse()
                    .map_err(|_| format!("argument {flag}: invalid int value: '{value}'"))?;
            }
            "--walsy-preview" => o.preview = Some(value),
            "--walsy-html" => o.html = Some(value),
            "--walsy-normals" => o.normals = Some(value),
            "--walsy-emissive" => o.emissive = Some(value),
            _ => return Err(format!("unrecognized arguments: {word}")),
        }
        i += 1;
    }
    if positional.is_empty() {
        return Err(if o.compose.is_some() {
            "the following arguments are required: out".into()
        } else {
            "the following arguments are required: sheet, out".into()
        });
    }
    if positional.len() == 1 {
        if o.compose.is_some() {
            o.sheet = "-".into();
            o.out = positional.remove(0);
            return Ok(Some(o));
        }
        return Err("the following arguments are required: out".into());
    }
    if positional.len() > 2 && !literal {
        return Err(format!(
            "unrecognized arguments: {}",
            positional[2..].join(" ")
        ));
    }
    o.sheet = positional.remove(0);
    o.out = positional.remove(0);
    Ok(Some(o))
}

fn check_dims(w: usize, h: usize) -> Result<usize, String> {
    // Platform-trap guard: huge dimensions would overflow buffers or exhaust memory.
    if w == 0 || h == 0 {
        return Err("bad image size".into());
    }
    if w > MAX_DIM || h > MAX_DIM {
        return Err("image too large".into());
    }
    w.checked_mul(h)
        .ok_or_else(|| "image too large".into())
}

fn next_token(bytes: &[u8], at: &mut usize) -> Result<Vec<u8>, String> {
    loop {
        while *at < bytes.len() && bytes[*at].is_ascii_whitespace() {
            *at += 1;
        }
        if *at >= bytes.len() {
            return Err("bad PPM".into());
        }
        if bytes[*at] == b'#' {
            while *at < bytes.len() && bytes[*at] != b'\n' {
                *at += 1;
            }
            continue;
        }
        break;
    }
    let start = *at;
    while *at < bytes.len() && !bytes[*at].is_ascii_whitespace() {
        *at += 1;
    }
    Ok(bytes[start..*at].to_vec())
}

fn token_number(token: &[u8]) -> Result<u64, String> {
    let text = std::str::from_utf8(token).map_err(|_| "bad PPM")?;
    text.parse::<u64>().map_err(|_| "bad PPM".into())
}

fn ppm_head(bytes: &[u8]) -> Result<(usize, usize, u64, usize, usize), String> {
    let mut at = 0;
    let magic = next_token(bytes, &mut at)?;
    if magic != b"P3" && magic != b"P6" {
        return Err("bad PPM".into());
    }
    let w = usize::try_from(token_number(&next_token(bytes, &mut at)?)?)
        .map_err(|_| "bad PPM")?;
    let h = usize::try_from(token_number(&next_token(bytes, &mut at)?)?)
        .map_err(|_| "bad PPM")?;
    let max = token_number(&next_token(bytes, &mut at)?)?;
    if max == 0 || max > 255 {
        return Err("bad PPM".into());
    }
    if bytes.get(at) == Some(&b'\r') && bytes.get(at + 1) == Some(&b'\n') {
        at += 2;
    } else if bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    } else {
        return Err("bad PPM".into());
    }
    let count = check_dims(w, h)?;
    Ok((w, h, max, at, count))
}

fn rescale(v: u8, max: u64) -> u8 {
    if max == 255 {
        return v;
    }
    ((u32::from(v) * 255 / max as u32).clamp(0, 255)) as u8
}

fn ppm_ascii(bytes: &[u8]) -> Result<(usize, usize, Vec<[u8; 4]>), String> {
    let (w, h, max, mut at, count) = ppm_head(bytes)?;
    let mut px = Vec::with_capacity(count);
    for _ in 0..count {
        let mut rgba = [0u8; 4];
        for channel in rgba.iter_mut().take(3) {
            let value = token_number(&next_token(bytes, &mut at)?)?;
            *channel = rescale(u8::try_from(value).map_err(|_| "bad PPM")?, max);
        }
        rgba[3] = 255;
        px.push(rgba);
    }
    Ok((w, h, px))
}

fn ppm_binary(bytes: &[u8]) -> Result<(usize, usize, Vec<[u8; 4]>), String> {
    let (w, h, max, mut at, count) = ppm_head(bytes)?;
    let mut px = Vec::with_capacity(count);
    for _ in 0..count {
        let mut rgba = [0u8; 4];
        for channel in rgba.iter_mut().take(3) {
            let value = *bytes.get(at).ok_or("bad PPM")?;
            at += 1;
            *channel = rescale(value, max);
        }
        rgba[3] = 255;
        px.push(rgba);
    }
    Ok((w, h, px))
}

fn at_byte(buf: &[u8], at: usize) -> Result<u8, String> {
    buf.get(at)
        .copied()
        .ok_or_else(|| "bad PNG".into())
}

fn decode_png(bytes: &[u8]) -> Result<(usize, usize, Vec<[u8; 4]>), String> {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let (w, h) = {
        let info = reader.info();
        (info.width as usize, info.height as usize)
    };
    check_dims(w, h)?;
    let size = reader.output_buffer_size().ok_or("bad PNG size")?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let at = (y * w + x) * info.color_type.samples();
            let rgba = match info.color_type {
                ColorType::Rgba => [
                    at_byte(&buf, at)?,
                    at_byte(&buf, at + 1)?,
                    at_byte(&buf, at + 2)?,
                    at_byte(&buf, at + 3)?,
                ],
                ColorType::Rgb => [
                    at_byte(&buf, at)?,
                    at_byte(&buf, at + 1)?,
                    at_byte(&buf, at + 2)?,
                    255,
                ],
                ColorType::GrayscaleAlpha => {
                    let g = at_byte(&buf, at)?;
                    [g, g, g, at_byte(&buf, at + 1)?]
                }
                ColorType::Grayscale => {
                    let g = at_byte(&buf, at)?;
                    [g, g, g, 255]
                }
                ColorType::Indexed => return Err("palette not expanded".into()),
            };
            px.push(rgba);
        }
    }
    Ok((w, h, px))
}

// shortcut: macOS-only raster conversion; add a cross-platform decoder when other hosts matter.
fn raster_to_png(path: &str) -> Result<Vec<u8>, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp = std::env::temp_dir().join(format!("walsy-{}-{nanos}.png", std::process::id()));
    let output = Command::new("sips")
        .arg("-s")
        .arg("format")
        .arg("png")
        .arg(path)
        .arg("--out")
        .arg(&temp)
        .output()
        .map_err(|e| format!("cannot convert {path} to PNG: {e}"))?;
    let png = if output.status.success() {
        fs::read(&temp).map_err(|e| format!("cannot read converted image: {e}"))
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(format!(
            "cannot convert {path} to PNG: {}",
            if detail.is_empty() {
                "unsupported image format".into()
            } else {
                detail
            }
        ))
    };
    let _ = fs::remove_file(&temp);
    png
}

fn load_image(path: &str) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let (w, h, pixels) = if bytes.starts_with(b"P3") && bytes.get(2).is_some_and(u8::is_ascii_whitespace)
    {
        ppm_ascii(&bytes)?
    } else if bytes.starts_with(b"P6") && bytes.get(2).is_some_and(u8::is_ascii_whitespace) {
        ppm_binary(&bytes)?
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        decode_png(&bytes)?
    } else {
        decode_png(&raster_to_png(path)?)?
    };
    let mut rows = Vec::with_capacity(h);
    for y in 0..h {
        let mut row = Vec::with_capacity(w);
        for x in 0..w {
            let px = pixels
                .get(y * w + x)
                .ok_or_else(|| "bad image size".to_owned())?;
            row.push(Value::Array(px.iter().map(|v| json!(v)).collect()));
        }
        rows.push(Value::Array(row));
    }
    Ok(Value::Array(rows))
}

fn write_image(path: &str, w: usize, h: usize, pixels: &[[u8; 4]]) -> Result<(), String> {
    fs::write(path, scripts_rs::engine::png_bytes(w, h, pixels)).map_err(|e| e.to_string())
}

fn sheet_value(w: usize, h: usize, pixels: &[[u8; 4]]) -> Value {
    Value::Array(
        (0..h)
            .map(|y| {
                Value::Array(
                    (0..w)
                        .map(|x| {
                            let px = pixels[y * w + x];
                            Value::Array(px.iter().map(|v| json!(v)).collect())
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

fn px_byte(px: &[Value], at: usize) -> Result<u8, String> {
    px.get(at)
        .and_then(Value::as_u64)
        .and_then(|n| u8::try_from(n).ok())
        .ok_or_else(|| "bad volume".into())
}

fn flatten(volume: &Value) -> Result<(usize, usize, Vec<[u8; 4]>), String> {
    let planes = volume.as_array().ok_or("bad volume")?;
    let rows = planes
        .first()
        .and_then(Value::as_array)
        .ok_or("bad volume")?;
    let (z, h, w) = (
        planes.len(),
        rows.len(),
        rows.first().and_then(Value::as_array).map_or(0, Vec::len),
    );
    let count = z
        .checked_mul(h)
        .and_then(|n| n.checked_mul(w))
        .ok_or("bad volume")?;
    let mut pixels = Vec::with_capacity(count);
    for y in 0..h {
        for plane in planes {
            let cols = plane.as_array().ok_or("bad volume")?;
            let row = cols.get(y).and_then(Value::as_array).ok_or("bad volume")?;
            if row.len() != w {
                return Err("bad volume".into());
            }
            for x in 0..w {
                let px = row.get(x).and_then(Value::as_array).ok_or("bad volume")?;
                pixels.push([
                    px_byte(px, 0)?,
                    px_byte(px, 1)?,
                    px_byte(px, 2)?,
                    px_byte(px, 3)?,
                ]);
            }
        }
    }
    Ok((z * w, h, pixels))
}

fn ensure_parent(path: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn execute(mut o: Options) -> Result<(), String> {
    let sheet = if let Some(kind) = o.compose.clone() {
        let d = scripts_rs::compose::defaults(&kind)
            .ok_or_else(|| format!("unknown compose preset: {kind}"))?;
        if !o.explicit.contains(&"--walsy-slices") {
            o.slices = Some(d.slices);
        }
        if !o.explicit.contains(&"--walsy-top-nose") {
            o.top_nose = d.top_nose.into();
        }
        if !o.explicit.contains(&"--walsy-side-nose") {
            o.side_nose = d.side_nose.into();
        }
        if !o.explicit.contains(&"--walsy-order") {
            o.order = d.order.into();
        }
        if !o.explicit.contains(&"--walsy-colours") {
            o.colours = d.colours;
        }
        if !o.explicit.contains(&"--walsy-shade") {
            o.shade = d.shade;
        }
        if !o.explicit.contains(&"--walsy-round") {
            o.rounded = d.rounded;
        }
        if !o.explicit.contains(&"--walsy-dark-thresh") {
            o.dark_thresh = d.dark_thresh;
        }
        if !o.explicit.contains(&"--walsy-bg-tol") {
            o.bg_tol = d.bg_tol;
        }
        let (w, h, pixels) = scripts_rs::compose::sheet(&kind)?;
        sheet_value(w, h, &pixels)
    } else {
        load_image(&o.sheet)?
    };
    let pair = scripts_rs::engine::views_to_slices(
        sheet,
        o.slices,
        o.top_nose,
        o.side_nose,
        o.order,
        o.colours,
        o.shade,
        o.dark_thresh,
        o.bg_tol,
        o.rounded,
    )?;
    let volume = pair.get(0).cloned().ok_or("bad volume")?;
    let views = pair
        .get(1)
        .and_then(Value::as_array)
        .cloned()
        .ok_or("bad views")?;
    let planes = volume.as_array().ok_or("bad volume")?;
    let z = planes.len();
    let h = planes.first().and_then(Value::as_array).map_or(0, Vec::len);
    let (strip_w, strip_h, pixels) = flatten(&volume)?;
    let slice_w = strip_w / z.max(1);
    ensure_parent(&o.out)?;
    write_image(&o.out, strip_w, strip_h, &pixels)?;
    println!("{z} slices of {slice_w} x {h} px -> {}", o.out);

    for run in scripts_rs::engine::thin_runs(volume.clone(), 3)? {
        let a = run
            .get(0)
            .and_then(Value::as_u64)
            .ok_or("bad run")?;
        let b = run
            .get(1)
            .and_then(Value::as_u64)
            .ok_or("bad run")?;
        println!(
            "warning: slices {a}-{b} hold a 1px-thin part; it will blink as the stack turns. Draw posts, ropes and antennas at least 2px wide in the views."
        );
    }

    if let Some(path) = &o.normals {
        ensure_parent(path)?;
        let norm = scripts_rs::engine::compute_normals(volume.clone())?;
        let (w, h, px) = flatten(&norm)?;
        write_image(path, w, h, &px)?;
        println!("normals -> {path}");
    }
    if let Some(path) = &o.emissive {
        ensure_parent(path)?;
        let glow = scripts_rs::engine::detect_emissive(volume.clone())?;
        let (w, h, px) = flatten(&glow)?;
        write_image(path, w, h, &px)?;
        println!("emissive -> {path}");
    }
    if let Some(path) = &o.preview {
        ensure_parent(path)?;
        scripts_rs::engine::build_preview(volume.clone(), json!(views.clone()), json!(path))?;
        println!("preview -> {path}");
    }
    if let Some(path) = &o.html {
        ensure_parent(path)?;
        let title = Path::new(&o.out)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        scripts_rs::engine::build_html_viewer(volume, json!(views), json!(path), title)?;
        println!("viewer -> {path}");
    }
    Ok(())
}

pub fn run(_matches: &ArgMatches) -> ExitCode {
    // Read argv directly so a literal `--` separator survives clap's pass-through.
    let raw: Vec<OsString> = std::env::args_os().skip(1).collect();
    let refs: Vec<&OsString> = raw.iter().collect();
    let opts = match parse(&refs) {
        Ok(Some(opts)) => opts,
        Ok(None) => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Err(message) => return fail(&message),
    };
    match execute(opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{NAME}: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn ppm_head_skips_comments() {
        let bytes = b"P6\n# 255 in comment\n2 1\n255\n\x00\xff\x20\x30\x40\x50";
        let (w, h, max, at, count) = ppm_head(bytes).unwrap();
        assert_eq!((w, h, max, at, count), (2, 1, 255, 28, 2));
        let (_, _, px) = ppm_binary(bytes).unwrap();
        assert_eq!(px, vec![[0, 255, 32, 255], [48, 64, 80, 255]]);
    }

    #[test]
    fn ppm_rejects_wide_and_zero_max() {
        assert_eq!(
            ppm_binary(b"P6\n1 1\n65535\n\x00\x00\x00"),
            Err("bad PPM".into())
        );
        assert_eq!(
            ppm_ascii(b"P3\n1 1\n0\n0 0 0"),
            Err("bad PPM".into())
        );
    }

    #[test]
    fn ppm_rejects_huge_dimensions() {
        assert_eq!(
            ppm_binary(b"P6\n10001 1\n255\n"),
            Err("image too large".into())
        );
    }

    #[test]
    fn parse_accepts_native_slice_count() {
        let args = raw(&["sheet", "out", "--walsy-slices", "0"]);
        let refs: Vec<&OsString> = args.iter().collect();
        let o = parse(&refs).unwrap().unwrap();
        assert_eq!(o.slices, Some(0));
        let args = raw(&["sheet", "out", "--walsy-slices", "-1"]);
        let refs: Vec<&OsString> = args.iter().collect();
        assert!(parse(&refs).is_err());
        let args = raw(&["sheet", "out", "--walsy-slices", "9223372036854775807"]);
        let refs: Vec<&OsString> = args.iter().collect();
        assert!(parse(&refs).is_err());
    }

    #[test]
    fn parse_accepts_abbreviations_and_rejects_ambiguous() {
        let args = raw(&["a", "b", "--sli=0", "--side=right", "--rou", "--", "-x"]);
        let refs: Vec<&OsString> = args.iter().collect();
        let o = parse(&refs).unwrap().unwrap();
        assert_eq!(o.slices, Some(0));
        assert_eq!(o.side_nose, "right");
        assert!(o.rounded);
        assert_eq!(o.sheet, "a");
        assert_eq!(o.out, "b");
        let args = raw(&["a", "b", "--walsy-s"]);
        let refs: Vec<&OsString> = args.iter().collect();
        assert!(parse(&refs).is_err());
    }

    #[test]
    fn parse_rejects_side_nose_auto() {
        let args = raw(&["a", "b", "--walsy-side-nose=auto"]);
        let refs: Vec<&OsString> = args.iter().collect();
        assert!(parse(&refs).is_err());
    }

    #[test]
    fn parse_help_and_usage_errors() {
        let args = raw(&["--he"]);
        let refs: Vec<&OsString> = args.iter().collect();
        assert!(parse(&refs).unwrap().is_none());
        let args = raw(&["a"]);
        let refs: Vec<&OsString> = args.iter().collect();
        assert_eq!(
            parse(&refs).err(),
            Some("the following arguments are required: out".into())
        );
    }

    #[test]
    fn flatten_rejects_ragged_volume() {
        let vol = json!([[[[0, 0, 0, 255]]], [[]]]);
        assert!(flatten(&vol).is_err());
    }
}
