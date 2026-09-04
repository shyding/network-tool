use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

#[derive(Default)]
pub struct TracertState {
    cancelled: AtomicBool,
}

fn parse_hop(line: &str) -> Option<Value> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    let hop = fields.first()?.parse::<u32>().ok()?;
    let timeout = fields.iter().filter(|field| **field == "*").count() >= 3;
    let ip = fields
        .iter()
        .rev()
        .find(|field| field.parse::<std::net::IpAddr>().is_ok())
        .copied();
    let mut times = Vec::new();
    for index in 1..fields.len() {
        let field = fields[index].trim_start_matches('<');
        if let Ok(value) = field.parse::<f64>() {
            if fields
                .get(index + 1)
                .is_some_and(|unit| unit.eq_ignore_ascii_case("ms") || *unit == "毫秒")
            {
                times.push(value);
            }
        }
    }
    let avg = (!times.is_empty()).then(|| times.iter().sum::<f64>() / times.len() as f64);
    Some(
        json!({"hop": hop, "ip": ip, "hostname": null, "times": times, "avg_ms": avg, "timeout": timeout, "line": line.trim()}),
    )
}

#[tauri::command]
pub async fn start_tracert(
    app: AppHandle,
    state: State<'_, TracertState>,
    host: String,
    max_hops: u32,
) -> Result<Value, String> {
    if host.trim().is_empty() {
        return Err("请输入目标主机".into());
    }
    state.cancelled.store(false, Ordering::Relaxed);
    let started = Instant::now();
    let mut child = Command::new("tracert.exe")
        .args([
            "-d",
            "-h",
            &max_hops.clamp(1, 255).to_string(),
            "-w",
            "1200",
            host.trim(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut stdout = child.stdout.take().ok_or("无法读取 tracert 输出")?;
    let mut pending = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut hops = Vec::new();
    loop {
        if state.cancelled.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            break;
        }
        let read = tokio::select! {
            result = stdout.read(&mut buffer) => result.map_err(|e|e.to_string())?,
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => continue,
        };
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&buffer[..read]);
        while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
            let bytes = pending.drain(..=end).collect::<Vec<_>>();
            let line = String::from_utf8_lossy(&bytes).trim().to_string();
            let _ = app.emit("tracert:line", &line);
            if let Some(hop) = parse_hop(&line) {
                let _ = app.emit("tracert:hop", &hop);
                hops.push(hop);
            }
        }
    }
    if !pending.is_empty() {
        let line = String::from_utf8_lossy(&pending).trim().to_string();
        if let Some(hop) = parse_hop(&line) {
            hops.push(hop);
        }
    }
    let status = child.wait().await.ok();
    let target_ip = hops
        .iter()
        .rev()
        .find_map(|hop| hop["ip"].as_str())
        .map(str::to_string);
    let reached = status.is_some_and(|value| value.success()) && !hops.is_empty();
    let result = json!({"target": host, "target_ip": target_ip, "reached": reached, "total_ms": started.elapsed().as_millis(), "hops": hops,"cancelled":state.cancelled.load(Ordering::Relaxed)});
    let _ = app.emit("tracert:complete", &result);
    Ok(result)
}

#[tauri::command]
pub fn stop_tracert(state: State<'_, TracertState>) -> bool {
    state.cancelled.store(true, Ordering::Relaxed);
    true
}
