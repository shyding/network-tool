use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct ConflictState {
    cancelled: AtomicBool,
}
fn addresses(range: &str) -> Result<Vec<std::net::Ipv4Addr>, String> {
    if let Ok(net) = range.parse::<ipnet::Ipv4Net>() {
        return Ok(net.hosts().take(65_536).collect());
    }
    if let Ok(ip) = range.parse::<std::net::Ipv4Addr>() {
        return Ok(vec![ip]);
    }
    if let Some((start, end)) = range.rsplit_once('-') {
        let base = start
            .parse::<std::net::Ipv4Addr>()
            .map_err(|_| "IP 范围格式无效")?;
        let last = end.parse::<u8>().map_err(|_| "IP 范围末尾必须是 0-255")?;
        let mut oct = base.octets();
        let begin = oct[3];
        if last < begin {
            return Err("IP 范围结束值不能小于起始值".into());
        }
        return Ok((begin..=last)
            .map(|n| {
                oct[3] = n;
                std::net::Ipv4Addr::from(oct)
            })
            .collect());
    }
    Err("IP 范围格式无效".into())
}
fn arp_macs(ip: &str) -> Vec<String> {
    let out = std::process::Command::new("arp.exe")
        .args(["-a", ip])
        .creation_flags(0x08000000)
        .output()
        .ok();
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut result = text
        .split_whitespace()
        .filter(|f| f.len() == 17 && f.matches('-').count() == 5)
        .map(|s| s.to_uppercase().replace('-', ":"))
        .collect::<Vec<_>>();
    result.sort();
    result.dedup();
    result
}
#[tauri::command]
pub async fn start_ip_conflict(
    app: AppHandle,
    state: State<'_, ConflictState>,
    range: String,
    iface_name: Option<String>,
    threads: usize,
    timeout_ms: u64,
) -> Result<Value, String> {
    let ips = addresses(&range)?;
    state.cancelled.store(false, Ordering::Relaxed);
    let local = crate::interfaces::all_interfaces()?
        .into_iter()
        .find(|i| iface_name.as_deref().is_none_or(|n| i["name"] == n));
    let local_ip = local
        .as_ref()
        .and_then(|i| i["ipv4"].as_str())
        .map(str::to_string);
    let local_mac = local
        .as_ref()
        .and_then(|i| i["mac"].as_str())
        .map(|s| s.to_uppercase().replace('-', ":"));
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(threads.clamp(1, 256)));
    let mut set = tokio::task::JoinSet::new();
    for ip in ips.iter().copied() {
        let permit = sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
        set.spawn(async move {
            let _p = permit;
            let ok = tokio::process::Command::new("ping.exe")
                .args([
                    "-n",
                    "1",
                    "-w",
                    &timeout_ms.clamp(100, 5000).to_string(),
                    &ip.to_string(),
                ])
                .creation_flags(0x08000000)
                .status()
                .await
                .is_ok_and(|s| s.success());
            (ip, ok)
        });
    }
    let started = Instant::now();
    let mut results = Vec::new();
    let mut scanned = 0;
    let mut conflicts = 0;
    while let Some(item) = set.join_next().await {
        if state.cancelled.load(Ordering::Relaxed) {
            set.abort_all();
            break;
        }
        let (ip, alive) = item.map_err(|e| e.to_string())?;
        scanned += 1;
        let text = ip.to_string();
        let macs = if alive { arp_macs(&text) } else { Vec::new() };
        let is_local = local_ip.as_deref() == Some(&text);
        let conflict = macs.len() > 1
            || (is_local
                && local_mac
                    .as_ref()
                    .is_some_and(|m| !macs.is_empty() && !macs.contains(m)));
        if conflict {
            conflicts += 1
        }
        let row = json!({"ip":text,"macs":macs,"status":if conflict{"conflict"}else if is_local{"local"}else if alive{"alive"}else{"empty"},"method":if alive{"Ping + ARP"}else{"Ping"},"detail":if conflict{"检测到 MAC 不一致或同 IP 多 MAC"}else if is_local{"本机地址"}else if alive{"主机有响应，未见冲突"}else{"无响应"}});
        results.push(row);
        let _ = app.emit(
            "ipconflict:progress",
            json!({"scanned":scanned,"total":ips.len(),"conflict_count":conflicts}),
        );
    }
    results.sort_by_key(|r| {
        r["ip"]
            .as_str()
            .and_then(|s| s.parse::<std::net::Ipv4Addr>().ok())
            .map(u32::from)
            .unwrap_or(0)
    });
    let alive = results.iter().filter(|r| r["status"] != "empty").count();
    Ok(
        json!({"range":range,"total":ips.len(),"scanned":scanned,"alive":alive,"conflict_count":conflicts,"results":results,"message":if conflicts==0{"未发现 IP 冲突"}else{"发现疑似 IP 冲突，请核对交换机与终端"},"elapsed_ms":started.elapsed().as_millis()}),
    )
}
#[tauri::command]
pub fn stop_ip_conflict(state: State<'_, ConflictState>) -> bool {
    state.cancelled.store(true, Ordering::Relaxed);
    true
}
