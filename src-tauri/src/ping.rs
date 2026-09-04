use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
use tokio::process::Command;

#[derive(Default)]
pub struct PingState {
    cancelled: AtomicBool,
}

async fn ping_one(host: &str, timeout_ms: u64, size: u32) -> (bool, Option<u64>, String) {
    let started = Instant::now();
    let output = Command::new("ping.exe")
        .args([
            "-n",
            "1",
            "-l",
            &size.to_string(),
            "-w",
            &timeout_ms.to_string(),
            host,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .output()
        .await;
    match output {
        Ok(result) => {
            let ok = result.status.success();
            let elapsed = ok.then(|| started.elapsed().as_millis() as u64);
            let line = if ok {
                format!("来自 {host} 的回复：时间={}ms", elapsed.unwrap_or_default())
            } else {
                format!("{host} 请求超时")
            };
            (ok, elapsed, line)
        }
        Err(error) => (false, None, format!("无法执行 ping：{error}")),
    }
}

fn summary(samples: &[Value]) -> Value {
    let sent = samples.len();
    let times = samples
        .iter()
        .filter_map(|row| row["time_ms"].as_u64())
        .collect::<Vec<_>>();
    let recv = times.len();
    let min = times.iter().min().copied();
    let max = times.iter().max().copied();
    let avg = (!times.is_empty()).then(|| times.iter().sum::<u64>() as f64 / times.len() as f64);
    json!({
        "sent": sent,
        "recv": recv,
        "loss_pct": if sent == 0 { 0.0 } else { (sent - recv) as f64 * 100.0 / sent as f64 },
        "min_ms": min,
        "max_ms": max,
        "avg_ms": avg.map(|value| (value * 10.0).round() / 10.0),
        "replies": samples,
    })
}

#[tauri::command]
pub async fn start_ping_once(
    app: AppHandle,
    state: State<'_, PingState>,
    host: String,
    count: u32,
    size: u32,
    timeout_ms: u64,
) -> Result<Value, String> {
    state.cancelled.store(false, Ordering::Relaxed);
    let mut samples = Vec::new();
    for index in 1..=count.min(100) {
        if state.cancelled.load(Ordering::Relaxed) {
            break;
        }
        let (ok, time_ms, line) = ping_one(&host, timeout_ms, size).await;
        let reply =
            json!({"index": index, "host": host, "ok": ok, "time_ms": time_ms, "line": line});
        let _ = app.emit("ping:line", json!({"line": line, "reply": reply}));
        samples.push(reply);
        if index < count {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
    let result = summary(&samples);
    let _ = app.emit("ping:complete", &result);
    Ok(result)
}

#[tauri::command]
pub async fn start_continuous_ping(
    app: AppHandle,
    state: State<'_, PingState>,
    host: String,
    size: u32,
    interval_sec: f64,
) -> Result<Value, String> {
    state.cancelled.store(false, Ordering::Relaxed);
    let mut samples = Vec::new();
    let mut seq = 1_u64;
    loop {
        if state.cancelled.load(Ordering::Relaxed) {
            break;
        }
        let (ok, time_ms, line) = ping_one(&host, 3000, size).await;
        let tick = json!({"seq": seq, "host": host, "ok": ok, "time_ms": time_ms, "line": line});
        let _ = app.emit("cping:tick", &tick);
        samples.push(tick);
        // The chart only keeps its latest 120 points, but the ping itself is
        // stop-driven.  Keep a bounded summary buffer so an overnight run does
        // not grow the process indefinitely.
        if samples.len() > 10_000 {
            samples.drain(..5_000);
        }
        seq = seq.saturating_add(1);
        tokio::time::sleep(Duration::from_secs_f64(interval_sec.clamp(0.1, 60.0))).await;
    }
    let result = summary(&samples);
    let _ = app.emit("cping:complete", &result);
    Ok(result)
}

#[tauri::command]
pub async fn start_tcp_ping(
    app: AppHandle,
    state: State<'_, PingState>,
    host: String,
    port: u16,
    count: u32,
    timeout_sec: f64,
) -> Result<Value, String> {
    state.cancelled.store(false, Ordering::Relaxed);
    let mut samples = Vec::new();
    for seq in 1..=count.min(100) {
        if state.cancelled.load(Ordering::Relaxed) {
            break;
        }
        let started = Instant::now();
        let connected = tokio::time::timeout(
            Duration::from_secs_f64(timeout_sec.clamp(0.1, 60.0)),
            tokio::net::TcpStream::connect((&*host, port)),
        )
        .await;
        let ok = matches!(connected, Ok(Ok(_)));
        let time_ms = ok.then(|| started.elapsed().as_millis() as u64);
        let line = if ok {
            format!("{host}:{port} 已连接")
        } else {
            format!("{host}:{port} 连接失败")
        };
        let tick = json!({"seq": seq, "host": host, "port": port, "ok": ok, "time_ms": time_ms, "line": line});
        let _ = app.emit("tping:tick", &tick);
        samples.push(tick);
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    let result = summary(&samples);
    let _ = app.emit("tping:complete", &result);
    Ok(result)
}

#[tauri::command]
pub fn stop_ping(state: State<'_, PingState>) -> bool {
    state.cancelled.store(true, Ordering::Relaxed);
    true
}

#[tauri::command]
pub async fn start_batch_ping(
    app: AppHandle,
    state: State<'_, PingState>,
    hosts: Vec<String>,
    timeout_ms: u64,
    threads: usize,
) -> Result<Value, String> {
    state.cancelled.store(false, Ordering::Relaxed);
    use std::sync::Arc;
    let mut rows = Vec::new();
    let concurrency = threads.clamp(1, 256);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut set = tokio::task::JoinSet::new();
    for (index, host) in hosts.into_iter().take(4096).enumerate() {
        if state.cancelled.load(Ordering::Relaxed) {
            break;
        }
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
        set.spawn(async move {
            let _permit = permit;
            let mut samples = Vec::new();
            for seq in 1..=2 {
                let (ok, time_ms, line) = ping_one(&host, timeout_ms, 32).await;
                samples.push(json!({"index": seq, "ok": ok, "time_ms": time_ms, "line": line}));
            }
            let stats = summary(&samples);
            let online = stats["recv"].as_u64().unwrap_or(0) > 0;
            json!({
                "index": index,
                "host": host,
                "online": online,
                "avg_ms": stats["avg_ms"], "min_ms": stats["min_ms"], "max_ms": stats["max_ms"],
                "loss_pct": stats["loss_pct"],
                "status": if online { "在线" } else { "离线" },
            })
        });
    }
    while let Some(result) = set.join_next().await {
        if state.cancelled.load(Ordering::Relaxed) {
            set.abort_all();
            break;
        }
        let row = result.map_err(|e| e.to_string())?;
        let _ = app.emit("bping:row", &row);
        rows.push(row);
    }
    rows.sort_by_key(|row| row["index"].as_u64().unwrap_or(u64::MAX));
    Ok(
        json!({"rows": rows, "total": rows.len(), "completed": !state.cancelled.load(Ordering::Relaxed), "cancelled": state.cancelled.load(Ordering::Relaxed)}),
    )
}

#[tauri::command]
pub async fn start_segment_ping(
    app: AppHandle,
    state: State<'_, PingState>,
    network: String,
    timeout_sec: f64,
    size: u32,
    threads: usize,
) -> Result<Value, String> {
    use std::sync::Arc;
    let net: ipnet::Ipv4Net = network
        .parse()
        .map_err(|_| "IPv4 CIDR 格式无效，例如 192.168.1.0/24")?;
    let addresses = net.hosts().take(65_536).collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("网段中没有可扫描地址".into());
    }
    state.cancelled.store(false, Ordering::Relaxed);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(threads.clamp(1, 200)));
    let mut set = tokio::task::JoinSet::new();
    for ip in addresses.iter().copied() {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
        let timeout = (timeout_sec.clamp(0.1, 10.0) * 1000.0) as u64;
        set.spawn(async move {
            let _p = permit;
            let started = Instant::now();
            let (alive, _, _) = ping_one(&ip.to_string(), timeout, size).await;
            (
                ip,
                alive,
                alive.then(|| started.elapsed().as_millis() as u64),
            )
        });
    }
    let total = addresses.len();
    let mut cells = Vec::with_capacity(total);
    let mut scanned = 0usize;
    let mut online = 0usize;
    while let Some(item) = set.join_next().await {
        if state.cancelled.load(Ordering::Relaxed) {
            set.abort_all();
            break;
        }
        let (ip, alive, time_ms) = item.map_err(|e| e.to_string())?;
        scanned += 1;
        if alive {
            online += 1
        }
        let host = u32::from(ip).saturating_sub(u32::from(net.network())) as usize;
        let cell = json!({"host":host,"ip":ip.to_string(),"alive":alive,"time_ms":time_ms});
        let _ = app.emit("sping:cell", &cell);
        let _=app.emit("sping:progress",json!({"scanned":scanned,"total":total,"online":online,"percent":scanned as f64*100.0/total as f64}));
        cells.push(cell);
    }
    cells.sort_by_key(|c| c["host"].as_u64().unwrap_or(0));
    Ok(
        json!({"network":network,"total":total,"scanned":scanned,"online":online,"cells":cells,"cancelled":state.cancelled.load(Ordering::Relaxed)}),
    )
}
