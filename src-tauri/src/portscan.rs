use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

#[derive(Default)]
pub struct PortScanState {
    cancelled: AtomicBool,
}

fn port_meta(port: u16) -> (&'static str, &'static str, &'static str) {
    match port {
        20 | 21 => ("FTP", "file", "medium"),
        22 => ("SSH", "remote", "medium"),
        23 => ("Telnet", "remote", "high"),
        25 | 465 | 587 => ("SMTP", "mail", "medium"),
        53 => ("DNS", "network", "low"),
        67 | 68 => ("DHCP", "network", "low"),
        80 | 8080 | 8000 => ("HTTP", "web", "low"),
        110 | 995 => ("POP3", "mail", "medium"),
        135 | 137 | 138 | 139 | 445 => ("Windows 服务", "windows", "high"),
        143 | 993 => ("IMAP", "mail", "medium"),
        161 => ("SNMP", "network", "medium"),
        389 | 636 => ("LDAP", "directory", "medium"),
        443 | 8443 => ("HTTPS", "web", "low"),
        1433 => ("MSSQL", "database", "high"),
        1521 => ("Oracle", "database", "high"),
        3306 => ("MySQL", "database", "high"),
        3389 => ("RDP", "remote", "high"),
        5432 => ("PostgreSQL", "database", "high"),
        5900 => ("VNC", "remote", "high"),
        6379 => ("Redis", "database", "high"),
        27017 => ("MongoDB", "database", "high"),
        _ => ("未知", "other", "low"),
    }
}

#[tauri::command]
pub async fn start_port_scan(
    app: AppHandle,
    state: State<'_, PortScanState>,
    host: String,
    start_port: u16,
    end_port: u16,
    threads: usize,
    timeout_ms: u64,
    use_common: bool,
) -> Result<Value, String> {
    if !use_common && start_port > end_port {
        return Err("端口范围无效".into());
    }
    state.cancelled.store(false, Ordering::Relaxed);
    let ports: Vec<u16> = if use_common {
        vec![
            20, 21, 22, 23, 25, 53, 67, 68, 80, 110, 135, 139, 143, 161, 389, 443, 445, 465, 587,
            636, 993, 995, 1433, 1521, 3306, 3389, 5432, 5900, 6379, 8000, 8080, 8443, 27017,
        ]
    } else {
        (start_port..=end_port).collect()
    };
    let total = ports.len();
    let semaphore = Arc::new(Semaphore::new(threads.clamp(1, 1024)));
    let cancelled = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let mut tasks = JoinSet::new();
    for port in ports {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| error.to_string())?;
        let host = host.clone();
        let cancelled = cancelled.clone();
        tasks.spawn(async move {
            let _permit = permit;
            if cancelled.load(Ordering::Relaxed) {
                return (port, false, 0);
            }
            let begin = Instant::now();
            let result = tokio::time::timeout(
                Duration::from_millis(timeout_ms.clamp(50, 30_000)),
                tokio::net::TcpStream::connect((&*host, port)),
            )
            .await;
            (
                port,
                matches!(result, Ok(Ok(_))),
                begin.elapsed().as_millis() as u64,
            )
        });
    }
    let mut scanned = 0usize;
    let mut open_ports = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        if state.cancelled.load(Ordering::Relaxed) {
            cancelled.store(true, Ordering::Relaxed);
            tasks.abort_all();
            break;
        }
        let (port, open, latency_ms) = joined.map_err(|error| error.to_string())?;
        scanned += 1;
        if open {
            let (service, category, risk) = port_meta(port);
            let row = json!({"port": port, "service": service, "category": category, "risk": risk,
                "latency_ms": latency_ms, "banner": "", "detail": format!("TCP {port} 可连接")});
            let _ = app.emit("portscan:open", &row);
            open_ports.push(row);
        }
        if scanned % 20 == 0 || scanned == total {
            let _ = app.emit(
                "portscan:progress",
                json!({"scanned": scanned, "total": total, "open": open_ports.len()}),
            );
        }
    }
    open_ports.sort_by_key(|row| row["port"].as_u64().unwrap_or(0));
    let result = json!({"host": host, "scanned": scanned, "total": total, "elapsed_ms": started.elapsed().as_millis(), "open_ports": open_ports});
    let _ = app.emit("portscan:complete", &result);
    Ok(result)
}

#[tauri::command]
pub fn stop_port_scan(state: State<'_, PortScanState>) -> bool {
    state.cancelled.store(true, Ordering::Relaxed);
    true
}
