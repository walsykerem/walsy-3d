use serde_json::{Value, json};
use std::{
    env,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::Command,
};

const PROTOCOL: &str = "2024-11-05";

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle(request),
            Err(_) => Some(error(Value::Null, -32700, "Parse error")),
        };
        if let Some(reply) = reply {
            let mut encoded = serde_json::to_string(&reply).unwrap();
            encoded.push('\n');
            let _ = stdout.write_all(encoded.as_bytes());
            let _ = stdout.flush();
        }
    }
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn handle(request: Value) -> Option<Value> {
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Some(error(request.get("id").cloned().unwrap_or(Value::Null), -32600, "Invalid Request"));
    }
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    if method.starts_with("notifications/") || request.get("id").is_none() {
        return None;
    }
    let result = match method {
        "initialize" => json!({
            "protocolVersion": PROTOCOL,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "walsy-mcp", "version": "0.1.0"}
        }),
        "tools/list" => json!({"tools": tools()}),
        "tools/call" => return Some(json!({"jsonrpc":"2.0","id":id,"result":call(&request)})),
        _ => return Some(error(id, -32601, "Method not found")),
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}

fn tools() -> Vec<Value> {
    let paths = json!({
        "type": "object",
        "properties": {
            "sheet": {"type": "string", "description": "Path to the four-view sheet image"},
            "out": {"type": "string", "description": "Path of the artifact to write"}
        },
        "required": ["sheet", "out"]
    });
    vec![
        json!({
            "name": "walsy_carve",
            "description": "Carve a slice strip from a four-view sheet",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sheet": {"type": "string", "description": "Path to the four-view sheet image"},
                    "out": {"type": "string", "description": "Path for the carved slice strip"},
                    "compose": {"type": "string", "enum": ["apple", "banana", "orange"], "description": "Generate a named four-view sheet before carving; pass \"-\" for sheet"},
                    "order": {"type": "string", "enum": ["auto", "reading"]},
                    "top_nose": {"type": "string", "enum": ["auto", "up", "down"]},
                    "side_nose": {"type": "string", "enum": ["auto", "left", "right"]},
                    "slices": {"type": "integer", "minimum": 0},
                    "shade": {"type": "number"},
                    "preview": {"type": "string", "description": "Path for the check sheet"},
                    "html": {"type": "string", "description": "Path for the HTML viewer"}
                },
                "required": ["sheet", "out"]
            }
        }),
        json!({
            "name": "walsy_lint",
            "description": "Run the walsy checks",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode": {"type": "string", "enum": ["check", "verify"]}
                },
                "required": ["mode"]
            }
        }),
        json!({
            "name": "walsy_preview",
            "description": "Write the four-view check sheet",
            "inputSchema": paths
        }),
        json!({
            "name": "walsy_viewer",
            "description": "Write an HTML turntable viewer",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sheet": {"type": "string", "description": "Path to the four-view sheet image"},
                    "out": {"type": "string", "description": "Path for the HTML viewer"}
                },
                "required": ["sheet", "out"]
            }
        }),
    ]
}

fn call(request: &Value) -> Value {
    let params = request.get("params");
    let name = params.and_then(|p| p.get("name")).and_then(Value::as_str);
    let args = params.and_then(|p| p.get("arguments"));
    let outcome = match (name, args) {
        (Some(name), Some(args)) if args.is_object() => match name {
            "walsy_carve" => carve(args),
            "walsy_preview" => artifact(args, "--walsy-preview", "preview"),
            "walsy_viewer" => artifact(args, "--walsy-html", "viewer"),
            "walsy_lint" => lint(args),
            _ => Err(format!("unknown tool: {name}")),
        },
        _ => Err("missing tool name or arguments".to_owned()),
    };
    let (text, is_error) = match outcome {
        Ok(text) => (text, false),
        Err(text) => (text, true),
    };
    json!({
        "content": [{"type": "text", "text": text}],
        "isError": is_error
    })
}

fn text_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("missing string argument: {key}"))
}

fn helper(env_key: &str, default_relative: &str) -> Result<PathBuf, String> {
    if let Ok(path) = env::var(env_key) {
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join(default_relative))
}

fn artifact_ok(path: &str) -> Result<u64, String> {
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("missing artifact {path}: {e}"))?;
    if !meta.is_file() || meta.len() == 0 {
        return Err(format!("empty artifact: {path}"));
    }
    Ok(meta.len())
}

fn run(binary: &Path, args: &[String]) -> Result<(bool, String), String> {
    let output = Command::new(binary)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run {}: {e}", binary.display()))?;
    Ok((output.status.success(), String::from_utf8_lossy(&output.stdout).into_owned()))
}

fn summary(stdout: &str) -> String {
    let line = stdout.lines().find(|line| line.contains(" slices of "));
    match line.and_then(|line| line.split_whitespace().next()) {
        Some(count) if count.chars().all(|c| c.is_ascii_digit()) => format!("{count} slices"),
        _ => "slice count unavailable".to_owned(),
    }
}

fn carve(args: &Value) -> Result<String, String> {
    let binary = helper("WALSY_CLI", "../app/target/release/views_to_slices")?;
    let compose = args.get("compose").and_then(Value::as_str).map(str::to_owned);
    let sheet = text_arg(args, "sheet")?;
    let out = text_arg(args, "out")?;
    let mut argv = if compose.is_some() {
        vec![out.clone()]
    } else {
        vec![sheet, out.clone()]
    };
    if let Some(kind) = compose {
        argv.push("--walsy-compose".to_owned());
        argv.push(kind);
    }
    for (key, flag) in [
        ("order", "--walsy-order"),
        ("top_nose", "--walsy-top-nose"),
        ("side_nose", "--walsy-side-nose"),
    ] {
        if let Some(value) = args.get(key).and_then(Value::as_str) {
            argv.push(flag.to_owned());
            argv.push(value.to_owned());
        }
    }
    if let Some(slices) = args.get("slices").and_then(Value::as_u64) {
        argv.push("--walsy-slices".to_owned());
        argv.push(slices.to_string());
    }
    if let Some(shade) = args.get("shade").and_then(Value::as_f64) {
        argv.push("--walsy-shade".to_owned());
        argv.push(shade.to_string());
    }
    let preview = args.get("preview").and_then(Value::as_str).map(str::to_owned);
    if let Some(path) = &preview {
        argv.push("--walsy-preview".to_owned());
        argv.push(path.clone());
    }
    let html = args.get("html").and_then(Value::as_str).map(str::to_owned);
    if let Some(path) = &html {
        argv.push("--walsy-html".to_owned());
        argv.push(path.clone());
    }
    let (ok, stdout) = run(&binary, &argv)?;
    if !ok {
        return Err("carve helper failed".to_owned());
    }
    let bytes = artifact_ok(&out)?;
    let mut details = vec![format!("strip {out} ({bytes} bytes)")];
    for (label, path) in [("preview", preview), ("viewer", html)] {
        if let Some(path) = path {
            let bytes = artifact_ok(&path)?;
            details.push(format!("{label} {path} ({bytes} bytes)"));
        }
    }
    Ok(format!("{}; {}", summary(&stdout), details.join("; ")))
}

fn artifact(args: &Value, flag: &str, label: &str) -> Result<String, String> {
    let binary = helper("WALSY_CLI", "../app/target/release/views_to_slices")?;
    let sheet = text_arg(args, "sheet")?;
    let out = text_arg(args, "out")?;
    let scratch = env::temp_dir().join(format!("walsy-mcp-{}-{label}.png", std::process::id()));
    let scratch = scratch.to_string_lossy().into_owned();
    let argv = vec![sheet, scratch.clone(), flag.to_owned(), out.clone()];
    let (ok, _) = run(&binary, &argv)?;
    let _ = std::fs::remove_file(&scratch);
    if !ok {
        return Err(format!("{label} helper failed"));
    }
    let bytes = artifact_ok(&out)?;
    Ok(format!("{label} {out} ({bytes} bytes)"))
}

fn lint(args: &Value) -> Result<String, String> {
    let mode = text_arg(args, "mode")?;
    if mode != "check" && mode != "verify" {
        return Err(format!("invalid mode: {mode}"));
    }
    let binary = helper("WALSY_LINT", "../lint-skill/target/release/lint-skill")?;
    let (ok, _) = run(&binary, &[mode.clone()])?;
    if !ok {
        return Err(format!("lint {mode} failed"));
    }
    Ok(format!("lint {mode} helper completed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_are_exactly_four() {
        let list = tools();
        let names: Vec<&str> = list.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["walsy_carve", "walsy_lint", "walsy_preview", "walsy_viewer"]);
    }

    #[test]
    fn carve_schema_offers_compose_presets() {
        let carve = tools()
            .into_iter()
            .find(|t| t["name"] == "walsy_carve")
            .unwrap();
        let compose = &carve["inputSchema"]["properties"]["compose"];
        assert_eq!(compose["enum"], json!(["apple", "banana", "orange"]));
    }

    #[test]
    fn notifications_get_no_reply() {
        let n = json!({"jsonrpc":"2.0","method":"notifications/cancelled"});
        assert!(handle(n).is_none());
    }

    #[test]
    fn slice_count_from_stdout() {
        assert_eq!(summary("36 slices of 30 x 65 px -> /tmp/s.png\npreview -> /tmp/c.png"), "36 slices");
        assert_eq!(summary(""), "slice count unavailable");
    }
}
