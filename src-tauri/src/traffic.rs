use serde_json::{json, Value};
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::State;

#[derive(Default)]
pub struct TrafficState {
    baseline: Mutex<Option<(Instant, HashMap<String, (u64, u64)>)>>,
}

fn counters() -> Result<HashMap<String, (u64, u64)>, String> {
    let script = r#"[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); @(Get-NetAdapterStatistics | ForEach-Object {[pscustomobject]@{name=$_.Name;rx=[uint64]$_.ReceivedBytes;tx=[uint64]$_.SentBytes}})|ConvertTo-Json -Compress"#;
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let rows = v.as_array().cloned().unwrap_or_default();
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some((
                r["name"].as_str()?.to_string(),
                (r["rx"].as_u64()?, r["tx"].as_u64()?),
            ))
        })
        .collect())
}

#[tauri::command]
pub fn reset_traffic_baseline(state: State<'_, TrafficState>) -> Result<bool, String> {
    *state.baseline.lock().map_err(|_| "流量状态锁异常")? = Some((Instant::now(), counters()?));
    Ok(true)
}

#[tauri::command]
pub fn sample_traffic(
    state: State<'_, TrafficState>,
    iface: Option<String>,
) -> Result<Value, String> {
    let now = Instant::now();
    let current = counters()?;
    let mut guard = state.baseline.lock().map_err(|_| "流量状态锁异常")?;
    let (elapsed, previous) = guard
        .as_ref()
        .map(|(t, c)| (now.duration_since(*t).as_secs_f64().max(0.001), c.clone()))
        .unwrap_or((1.0, current.clone()));
    let selected = iface
        .as_deref()
        .and_then(|name| current.get(name).map(|v| (name.to_string(), *v)))
        .or_else(|| current.iter().next().map(|(n, v)| (n.clone(), *v)));
    let sample=selected.map(|(name,(rx,tx))|{let (prx,ptx)=previous.get(&name).copied().unwrap_or((rx,tx));json!({"iface":name,"ts_ms":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),"rx_bps":((rx.saturating_sub(prx)) as f64/elapsed),"tx_bps":((tx.saturating_sub(ptx)) as f64/elapsed),"rx_total":rx,"tx_total":tx})});
    *guard = Some((now, current));
    Ok(json!({"ifaces":crate::interfaces::all_interfaces()?,"sample":sample}))
}
