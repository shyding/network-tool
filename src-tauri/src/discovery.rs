use serde_json::{json, Value};
use std::collections::HashSet;
use std::os::windows::process::CommandExt;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct DiscoveryState {
    cancelled: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    host_slots: Arc<tokio::sync::Semaphore>,
    socket_slots: Arc<tokio::sync::Semaphore>,
}

impl Default for DiscoveryState {
    fn default() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            host_slots: Arc::new(tokio::sync::Semaphore::new(4)),
            socket_slots: Arc::new(tokio::sync::Semaphore::new(384)),
        }
    }
}

const QUICK_PORTS: [u16; 24] = [
    21, 22, 23, 25, 53, 80, 110, 135, 139, 143, 443, 445, 554, 1433, 1521, 3306, 3389,
    5432, 5900, 6379, 8080, 8443, 9100, 27017,
];
const FULL_PORT_TOTAL: usize = 65_535;

#[derive(Clone)]
struct DetectedService {
    service: &'static str,
    category: &'static str,
    scheme: Option<&'static str>,
    evidence: String,
}

#[derive(Clone)]
struct PortFinding {
    port: u16,
    latency_ms: u64,
    detected: Option<DetectedService>,
}

async fn ping_ip(ip: std::net::Ipv4Addr, timeout: u64) -> bool {
    tokio::process::Command::new("ping.exe")
        .args(["-n", "1", "-w", &timeout.to_string(), &ip.to_string()])
        .creation_flags(0x08000000)
        .status()
        .await
        .is_ok_and(|s| s.success())
}

async fn tcp_open(ip: std::net::Ipv4Addr, port: u16, timeout_ms: u64) -> Option<u64> {
    let started = Instant::now();
    tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        tokio::net::TcpStream::connect((ip, port)),
    )
    .await
    .ok()?
    .ok()?;
    Some(started.elapsed().as_millis().max(1) as u64)
}

async fn detect_open_service(
    ip: std::net::Ipv4Addr,
    port: u16,
    timeout_ms: u64,
) -> Option<DetectedService> {
    let mut stream = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        tokio::net::TcpStream::connect((ip, port)),
    )
    .await
    .ok()?
    .ok()?;
    let mut buffer = [0u8; 1024];

    if let Ok(Ok(size)) = tokio::time::timeout(
        Duration::from_millis(120),
        stream.read(&mut buffer),
    )
    .await
    {
        if size > 0 {
            let banner = String::from_utf8_lossy(&buffer[..size]);
            let lower = banner.to_ascii_lowercase();
            if banner.starts_with("SSH-") {
                return Some(DetectedService { service: "SSH", category: "远程管理", scheme: None, evidence: banner.lines().next().unwrap_or("SSH banner").chars().take(120).collect() });
            }
            if lower.contains("ftp") && banner.starts_with("220") {
                return Some(DetectedService { service: "FTP", category: "文件传输", scheme: None, evidence: banner.lines().next().unwrap_or("FTP banner").chars().take(120).collect() });
            }
            if size > 5 && buffer[4] == 0x0a && (lower.contains("mysql") || lower.contains("mariadb")) {
                return Some(DetectedService { service: "MySQL", category: "数据库", scheme: None, evidence: "MySQL protocol handshake".into() });
            }
        }
    }

    let request = format!("HEAD / HTTP/1.0\r\nHost: {ip}:{port}\r\nConnection: close\r\n\r\n");
    tokio::time::timeout(Duration::from_millis(200), stream.write_all(request.as_bytes()))
        .await
        .ok()?
        .ok()?;
    let size = tokio::time::timeout(Duration::from_millis(timeout_ms), stream.read(&mut buffer))
        .await
        .ok()?
        .ok()?;
    let response = String::from_utf8_lossy(&buffer[..size]);
    if response.starts_with("HTTP/") {
        return Some(DetectedService {
            service: "HTTP",
            category: "Web",
            scheme: Some("http"),
            evidence: response.lines().next().unwrap_or("HTTP response").chars().take(120).collect(),
        });
    }
    None
}

async fn quick_tcp_probe(ip: std::net::Ipv4Addr, timeout_ms: u64) -> Vec<PortFinding> {
    let mut tasks = tokio::task::JoinSet::new();
    for port in QUICK_PORTS {
        tasks.spawn(async move {
            let Some(latency_ms) = tcp_open(ip, port, timeout_ms).await else { return None };
            let detected = detect_open_service(ip, port, timeout_ms).await;
            Some(PortFinding { port, latency_ms, detected })
        });
    }
    let mut ports = Vec::new();
    while let Some(Ok(finding)) = tasks.join_next().await {
        if let Some(finding) = finding { ports.push(finding) }
    }
    ports.sort_unstable_by_key(|item| item.port);
    ports
}

async fn probe(
    ip: std::net::Ipv4Addr,
    method: &str,
    timeout: u64,
) -> (bool, Option<u64>, Vec<PortFinding>, String) {
    let started = Instant::now();
    // This phase exists to discover hosts, not to wait for the full port sweep.
    // LAN TCP handshakes normally complete in milliseconds; a short ceiling keeps
    // filtered addresses from multiplying the foreground scan duration.
    let tcp_timeout = timeout.clamp(100, 350);
    let (ping_alive, ports) = match method {
        "ping" => (ping_ip(ip, timeout).await, Vec::new()),
        "hybrid" => tokio::join!(ping_ip(ip, timeout), quick_tcp_probe(ip, tcp_timeout)),
        _ => (false, quick_tcp_probe(ip, tcp_timeout).await),
    };
    let alive = ping_alive || !ports.is_empty();
    let elapsed = alive.then(|| started.elapsed().as_millis() as u64);
    let method_hit = match (ping_alive, ports.is_empty()) {
        (true, false) => "ping+tcp",
        (true, true) => "ping",
        (false, false) => "tcp",
        (false, true) => method,
    };
    (
        alive,
        elapsed,
        ports,
        method_hit.into(),
    )
}
fn arp_mac(ip: &str) -> Option<String> {
    let out = std::process::Command::new("arp.exe")
        .args(["-a", ip])
        .creation_flags(0x08000000)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .flat_map(|l| l.split_whitespace())
        .find(|f| f.len() == 17 && f.matches('-').count() == 5)
        .map(|s| s.to_uppercase().replace('-', ":"))
}
fn role(ports: &[u16], is_gateway: bool) -> &'static str {
    if is_gateway {
        "gateway"
    } else if ports.contains(&9100) {
        "printer"
    } else if ports.contains(&554) {
        "camera"
    } else if ports.contains(&445) {
        "nas"
    } else if ports.contains(&80) || ports.contains(&443) {
        "network"
    } else {
        "host"
    }
}

fn port_service(port: u16) -> (&'static str, &'static str) {
    match port {
        20 => ("FTP-DATA", "文件传输"),
        21 => ("FTP", "文件传输"),
        22 => ("SSH", "远程管理"),
        23 => ("Telnet", "远程管理"),
        25 => ("SMTP", "邮件"),
        53 => ("DNS", "基础服务"),
        80 => ("HTTP", "Web"),
        110 => ("POP3", "邮件"),
        135 => ("MS-RPC", "Windows"),
        137..=139 => ("NetBIOS", "Windows"),
        143 => ("IMAP", "邮件"),
        389 => ("LDAP", "目录服务"),
        443 => ("HTTPS", "Web"),
        445 => ("SMB", "文件共享"),
        465 => ("SMTPS", "邮件"),
        554 => ("RTSP", "视频监控"),
        587 => ("SMTP Submission", "邮件"),
        631 => ("IPP", "打印"),
        873 => ("rsync", "文件同步"),
        993 => ("IMAPS", "邮件"),
        995 => ("POP3S", "邮件"),
        1433 => ("MSSQL", "数据库"),
        1521 => ("Oracle", "数据库"),
        2049 => ("NFS", "文件共享"),
        2375 | 2376 => ("Docker", "容器"),
        3306 => ("MySQL", "数据库"),
        3389 => ("RDP", "远程管理"),
        5432 => ("PostgreSQL", "数据库"),
        5900..=5909 => ("VNC", "远程管理"),
        6379 => ("Redis", "数据库"),
        8000 | 8008 | 8080 | 8081 | 8088 | 8888 => ("HTTP-ALT", "Web"),
        8443 => ("HTTPS-ALT", "Web"),
        9100 => ("JetDirect", "打印"),
        9200 => ("Elasticsearch", "数据库"),
        11211 => ("Memcached", "数据库"),
        27017 => ("MongoDB", "数据库"),
        _ => ("未知服务", "其他"),
    }
}

fn port_info(ip: &str, finding: &PortFinding) -> Value {
    let (hint_service, hint_category) = port_service(finding.port);
    let (service, category, url, verified, evidence) = match &finding.detected {
        Some(detected) => (
            detected.service,
            detected.category,
            detected.scheme.map(|scheme| format!("{scheme}://{ip}:{}", finding.port)),
            true,
            Some(detected.evidence.clone()),
        ),
        None => (hint_service, hint_category, None, false, None),
    };
    json!({
        "port":finding.port,"state":"open","service":service,"category":category,
        "latency_ms":finding.latency_ms,"url":url,"verified":verified,"evidence":evidence
    })
}

fn scan_active(cancelled: &AtomicBool, generation: &AtomicU64, expected: u64) -> bool {
    !cancelled.load(Ordering::Relaxed) && generation.load(Ordering::Relaxed) == expected
}

#[allow(clippy::too_many_arguments)]
async fn full_port_scan(
    app: AppHandle,
    mut host: Value,
    initial_ports: Vec<PortFinding>,
    timeout_ms: u64,
    expected_generation: u64,
    cancelled: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    host_slots: Arc<tokio::sync::Semaphore>,
    socket_slots: Arc<tokio::sync::Semaphore>,
) {
    let Ok(_host_slot) = host_slots.acquire_owned().await else { return };
    if !scan_active(&cancelled, &generation, expected_generation) { return }
    let Some(ip_text) = host["ip"].as_str().map(str::to_owned) else { return };
    let Ok(ip) = ip_text.parse::<std::net::Ipv4Addr>() else { return };
    let started = Instant::now();
    let mut open_ports = initial_ports.iter()
        .map(|finding| port_info(&ip_text, finding))
        .collect::<Vec<_>>();
    let quick_set = QUICK_PORTS.iter().copied().collect::<HashSet<_>>();
    let mut candidates = (1u16..=u16::MAX).filter(|port| !quick_set.contains(port));
    let mut tasks = tokio::task::JoinSet::new();
    let mut completed = quick_set.len();
    host["open_ports"] = json!(open_ports);
    host["port_scan"] = json!({"state":"scanning","scanned":completed,"total":FULL_PORT_TOTAL,"percent":completed as f64*100.0/FULL_PORT_TOTAL as f64,"open_count":open_ports.len(),"elapsed_ms":0});
    let _ = app.emit("hostdisc:host", &host);
    let _ = app.emit("hostdisc:line", format!("{ip_text} · 后台全端口扫描 1-65535 已启动（主机发现不等待）"));

    loop {
        while tasks.len() < 128 {
            let Some(port) = candidates.next() else { break };
            let slots = socket_slots.clone();
            tasks.spawn(async move {
                let Ok(_slot) = slots.acquire_owned().await else { return None };
                let Some(latency_ms) = tcp_open(ip, port, timeout_ms).await else { return None };
                let detected = detect_open_service(ip, port, timeout_ms).await;
                Some(PortFinding { port, latency_ms, detected })
            });
        }
        if tasks.is_empty() { break }
        if !scan_active(&cancelled, &generation, expected_generation) {
            tasks.abort_all();
            host["port_scan"]["state"] = json!("cancelled");
            let _ = app.emit("hostdisc:host", &host);
            return;
        }
        let Some(result) = tasks.join_next().await else { break };
        completed += 1;
        let mut found = false;
        if let Ok(Some(finding)) = result {
            open_ports.push(port_info(&ip_text, &finding));
            open_ports.sort_by_key(|item| item["port"].as_u64().unwrap_or(0));
            found = true;
            let service = finding.detected.as_ref().map(|item| item.service).unwrap_or_else(|| port_service(finding.port).0);
            let verified = if finding.detected.is_some() { "协议已确认" } else { "端口推断" };
            let _ = app.emit("hostdisc:line", format!("{ip_text} · 开放端口 {} · {service} · {verified}", finding.port));
        }
        if found || completed % 512 == 0 || completed == FULL_PORT_TOTAL {
            let numeric = open_ports.iter().filter_map(|item| item["port"].as_u64().map(|p| p as u16)).collect::<Vec<_>>();
            host["role"] = json!(role(&numeric, host["is_gateway"].as_bool().unwrap_or(false)));
            host["open_ports"] = json!(open_ports);
            host["port_scan"] = json!({
                "state":if completed >= FULL_PORT_TOTAL {"complete"} else {"scanning"},
                "scanned":completed.min(FULL_PORT_TOTAL),"total":FULL_PORT_TOTAL,
                "percent":completed.min(FULL_PORT_TOTAL) as f64*100.0/FULL_PORT_TOTAL as f64,
                "open_count":open_ports.len(),"elapsed_ms":started.elapsed().as_millis()
            });
            let _ = app.emit("hostdisc:host", &host);
        }
    }
    let _ = app.emit("hostdisc:line", format!("{ip_text} · 全端口扫描完成 · 开放 {} 个 · {:.1}s", open_ports.len(), started.elapsed().as_secs_f64()));
}

#[tauri::command]
pub async fn start_host_discovery(
    app: AppHandle,
    state: State<'_, DiscoveryState>,
    network: String,
    scan_method: String,
    threads: usize,
    timeout_ms: u64,
) -> Result<Value, String> {
    let net: ipnet::Ipv4Net = network
        .parse()
        .map_err(|_| "IPv4 CIDR 格式无效，例如 192.168.1.0/24")?;
    let addresses = net.hosts().take(65_536).collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("网段内没有可扫描地址".into());
    }
    state.cancelled.store(false, Ordering::Relaxed);
    let run_generation = state.generation.fetch_add(1, Ordering::Relaxed) + 1;
    let started = Instant::now();
    let mut set = tokio::task::JoinSet::new();
    let mut pending = addresses.iter().copied();
    for ip in pending.by_ref().take(threads.clamp(1, 256)) {
        let method = scan_method.clone();
        set.spawn(async move {
            let result = probe(ip, &method, timeout_ms.clamp(100, 10_000)).await;
            (ip, result)
        });
    }
    let gateway = crate::interfaces::all_interfaces()
        .ok()
        .and_then(|v| v.into_iter().find(|i| i["is_lan_primary"] == true))
        .and_then(|i| i["gateway"].as_str().map(str::to_string));
    let total = addresses.len();
    let mut scanned = 0usize;
    let mut hosts = Vec::new();
    let mut full_scan_jobs = Vec::new();
    while let Some(joined) = set.join_next().await {
        if !scan_active(&state.cancelled, &state.generation, run_generation) {
            set.abort_all();
            break;
        }
        let (ip, (alive, time_ms, ports, method_hit)) = joined.map_err(|e| e.to_string())?;
        scanned += 1;
        if alive {
            let ip_text = ip.to_string();
            let is_gateway = gateway.as_deref() == Some(&ip_text);
            let mac = arp_mac(&ip_text);
            let numeric_ports = ports.iter().map(|item| item.port).collect::<Vec<_>>();
            let port_rows = ports.iter().map(|finding| port_info(&ip_text, finding)).collect::<Vec<_>>();
            let host = json!({
                "ip":ip_text.clone(),"alive":true,"response_time_ms":time_ms,"hostname":null,
                "mac":mac,"vendor":"未知","role":role(&numeric_ports,is_gateway),
                "is_gateway":is_gateway,"method_hit":method_hit,"open_ports":port_rows,
                "port_scan":{"state":"queued","scanned":QUICK_PORTS.len(),"total":FULL_PORT_TOTAL,
                    "percent":QUICK_PORTS.len() as f64*100.0/FULL_PORT_TOTAL as f64,
                    "open_count":numeric_ports.len(),"elapsed_ms":0}
            });
            let _ = app.emit("hostdisc:host", &host);
            let dns_app = app.clone();
            let dns_ip = ip_text.clone();
            tauri::async_runtime::spawn(async move {
                let resolved = tokio::task::spawn_blocking(move || {
                    dns_lookup::lookup_addr(&std::net::IpAddr::V4(ip)).ok()
                })
                .await
                .unwrap_or(None);
                if let Some(hostname) = resolved {
                    let _ = dns_app.emit("hostdisc:host", json!({"ip":dns_ip,"hostname":hostname}));
                }
            });
            full_scan_jobs.push((host.clone(), ports));
            hosts.push(host);
        }
        let _=app.emit("hostdisc:progress",json!({"scanned":scanned,"total":total,"alive_count":hosts.len(),"percent":scanned as f64*100.0/total as f64}));
        if let Some(next_ip) = pending.next() {
            let method = scan_method.clone();
            set.spawn(async move {
                let result = probe(next_ip, &method, timeout_ms.clamp(100, 10_000)).await;
                (next_ip, result)
            });
        }
    }
    hosts.sort_by_key(|h| {
        h["ip"]
            .as_str()
            .and_then(|s| s.parse::<std::net::Ipv4Addr>().ok())
            .map(u32::from)
            .unwrap_or(0)
    });
    let gateway_host = hosts
        .iter()
        .find(|h| h["is_gateway"] == true)
        .cloned()
        .unwrap_or_else(|| json!({"ip":gateway,"mac":null,"vendor":"未知"}));
    let cancelled = !scan_active(&state.cancelled, &state.generation, run_generation);
    let result = json!({"network":network,"total":total,"scanned":scanned,"alive_count":hosts.len(),"elapsed_ms":started.elapsed().as_millis(),"hosts":hosts,"gateway":gateway_host,"cancelled":cancelled,"port_scan_running":!cancelled&&!full_scan_jobs.is_empty()});
    let _ = app.emit("hostdisc:complete", &result);
    if !cancelled {
        for (host, ports) in full_scan_jobs {
            tauri::async_runtime::spawn(full_port_scan(
                app.clone(), host, ports, (timeout_ms / 4).clamp(250, 1_000), run_generation,
                state.cancelled.clone(), state.generation.clone(), state.host_slots.clone(),
                state.socket_slots.clone(),
            ));
        }
    }
    Ok(result)
}

#[tauri::command]
pub fn stop_host_discovery(state: State<'_, DiscoveryState>) -> bool {
    state.cancelled.store(true, Ordering::Relaxed);
    state.generation.fetch_add(1, Ordering::Relaxed);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_services_are_named() {
        assert_eq!(port_service(21).0, "FTP");
        assert_eq!(port_service(22).0, "SSH");
        assert_eq!(port_service(80).0, "HTTP");
        assert_eq!(port_service(3306).0, "MySQL");
    }

    #[tokio::test]
    async fn detects_http_on_an_arbitrary_port_and_builds_url() {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 512];
            let _ = socket.read(&mut request).await;
            socket.write_all(b"HTTP/1.1 200 OK\r\nServer: test\r\nContent-Length: 0\r\n\r\n").await.unwrap();
        });
        let detected = detect_open_service(std::net::Ipv4Addr::LOCALHOST, port, 500).await.unwrap();
        assert_eq!(detected.service, "HTTP");
        let finding = PortFinding { port, latency_ms: 1, detected: Some(detected) };
        let row = port_info("127.0.0.1", &finding);
        assert_eq!(row["verified"], true);
        assert_eq!(row["url"], format!("http://127.0.0.1:{port}"));
    }
}
