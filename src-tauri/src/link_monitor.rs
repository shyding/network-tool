use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct LinkMonitorState {
    running: Arc<AtomicBool>,
}
#[tauri::command]
pub fn link_monitor_running(state: State<'_, LinkMonitorState>) -> bool {
    state.running.load(Ordering::Relaxed)
}
#[tauri::command]
pub fn stop_link_monitor(state: State<'_, LinkMonitorState>) -> bool {
    state.running.store(false, Ordering::Relaxed);
    true
}
#[tauri::command]
pub fn start_link_monitor(
    app: AppHandle,
    state: State<'_, LinkMonitorState>,
    targets: Vec<String>,
    interval_sec: f64,
    timeout_ms: u64,
    lat_th: f64,
    loss_th: f64,
    jit_th: f64,
) -> Result<bool, String> {
    if targets.is_empty() {
        return Err("监控目标不能为空".into());
    }
    state.running.store(false, Ordering::Relaxed);
    let running = state.running.clone();
    running.store(true, Ordering::Relaxed);
    std::thread::spawn(move || {
        let mut previous = std::collections::HashMap::<String, f64>::new();
        let mut history = std::collections::HashMap::<String, Vec<f64>>::new();
        while running.load(Ordering::Relaxed) {
            for target in &targets {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                let started = Instant::now();
                let ok = std::process::Command::new("ping.exe")
                    .args([
                        "-n",
                        "1",
                        "-w",
                        &timeout_ms.clamp(100, 60_000).to_string(),
                        target,
                    ])
                    .creation_flags(0x08000000)
                    .status()
                    .is_ok_and(|s| s.success());
                let last = if ok {
                    Some(started.elapsed().as_secs_f64() * 1000.0)
                } else {
                    None
                };
                let samples = history.entry(target.clone()).or_default();
                samples.push(last.unwrap_or(timeout_ms as f64));
                if samples.len() > 20 {
                    samples.remove(0);
                }
                let avg = samples.iter().sum::<f64>() / samples.len() as f64;
                let loss = samples.iter().filter(|v| **v >= timeout_ms as f64).count() as f64
                    * 100.0
                    / samples.len() as f64;
                let jitter = last
                    .and_then(|v| previous.insert(target.clone(), v).map(|p| (v - p).abs()))
                    .unwrap_or(0.0);
                let tick = json!({"target":target,"ok":ok,"last_ms":last,"avg_ms":avg,"loss_pct":loss,"jitter_ms":jitter,"ts_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()});
                let _ = app.emit("linkmon:tick", &tick);
                if !ok || last.is_some_and(|v| v > lat_th) || loss > loss_th || jitter > jit_th {
                    let _=app.emit("linkmon:alert",json!({"target":target,"level":if !ok{"down"}else{"warn"},"message":if !ok{"目标不可达"}else{"时延/丢包/抖动超过阈值"},"sample":tick}));
                }
            }
            std::thread::sleep(Duration::from_secs_f64(interval_sec.clamp(0.2, 3600.0)));
        }
        let _ = app.emit("linkmon:stopped", Value::Null);
    });
    Ok(true)
}
