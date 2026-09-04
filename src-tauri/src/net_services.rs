use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, UdpSocket};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

struct Service {
    running: Arc<AtomicBool>,
    count: Arc<AtomicU64>,
    address: String,
}
#[derive(Default)]
pub struct NetServiceState {
    services: Mutex<HashMap<String, Service>>,
    dhcp_leases: Arc<Mutex<Vec<Value>>>,
    radius_accept: Arc<AtomicU64>,
    radius_reject: Arc<AtomicU64>,
    radius_users: Arc<AtomicU64>,
}
fn install(
    state: &NetServiceState,
    kind: &str,
    address: String,
    running: Arc<AtomicBool>,
    count: Arc<AtomicU64>,
) -> Result<bool, String> {
    let mut map = state.services.lock().map_err(|_| "服务状态锁异常")?;
    if let Some(old) = map.remove(kind) {
        old.running.store(false, Ordering::Relaxed);
    }
    map.insert(kind.into(), Service { running, count, address });
    Ok(true)
}
fn log(app: &AppHandle, text: String) {
    let _ = app.emit("netsvc:log", text);
}
fn start_udp(
    app: AppHandle,
    state: &NetServiceState,
    kind: &str,
    host: &str,
    port: u16,
) -> Result<bool, String> {
    let socket = UdpSocket::bind((host, port))
        .map_err(|e| format!("{kind} 绑定 {host}:{port} 失败：{e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(250)))
        .ok();
    let running = Arc::new(AtomicBool::new(true));
    let count = Arc::new(AtomicU64::new(0));
    install(state, kind, format!("{host}:{port}"), running.clone(), count.clone())?;
    let label = kind.to_string();
    std::thread::spawn(move || {
        let mut buf = [0u8; 65535];
        log(&app, format!("{label} 服务已启动"));
        while running.load(Ordering::Relaxed) {
            if let Ok((n, from)) = socket.recv_from(&mut buf) {
                count.fetch_add(1, Ordering::Relaxed);
                log(&app, format!("{label} 收到 {from} 的 {n} 字节"));
                if label == "radius" && n >= 20 {
                    let mut reply = buf[..20].to_vec();
                    reply[0] = 3;
                    let _ = socket.send_to(&reply, from);
                } else if label == "tftp" && n >= 2 {
                    let msg = [
                        0u8, 5, 0, 4, b'N', b'o', b't', b' ', b'f', b'i', b'l', b'e', 0,
                    ];
                    let _ = socket.send_to(&msg, from);
                }
            }
        }
        log(&app, format!("{label} 服务已停止"));
    });
    Ok(true)
}

fn safe_path(root: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    let relative = name.replace('\\', "/");
    if relative.split('/').any(|part| part == "..") {
        return Err("拒绝访问服务根目录之外的路径".into());
    }
    Ok(root.join(relative.trim_start_matches('/')))
}

fn service_root(root: &str) -> PathBuf {
    let requested = PathBuf::from(root);
    if requested.is_absolute() {
        return requested;
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        .join(requested)
}

fn start_tftp(
    app: AppHandle,
    state: &NetServiceState,
    host: &str,
    port: u16,
    root: PathBuf,
) -> Result<bool, String> {
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let socket = UdpSocket::bind((host, port)).map_err(|e| format!("TFTP 绑定失败：{e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(250)))
        .ok();
    let running = Arc::new(AtomicBool::new(true));
    let count = Arc::new(AtomicU64::new(0));
    install(state, "tftp", format!("{host}:{port}"), running.clone(), count.clone())?;
    std::thread::spawn(move || {
        log(&app, format!("TFTP 服务已启动，目录 {}", root.display()));
        let mut request = [0_u8; 2048];
        while running.load(Ordering::Relaxed) {
            let Ok((n, peer)) = socket.recv_from(&mut request) else {
                continue;
            };
            count.fetch_add(1, Ordering::Relaxed);
            if n < 4 {
                continue;
            }
            let opcode = u16::from_be_bytes([request[0], request[1]]);
            let end = request[2..n]
                .iter()
                .position(|b| *b == 0)
                .map(|p| p + 2)
                .unwrap_or(n);
            let name = String::from_utf8_lossy(&request[2..end]).into_owned();
            let Ok(path) = safe_path(&root, &name) else {
                continue;
            };
            let app2 = app.clone();
            let live = running.clone();
            std::thread::spawn(move || tftp_transfer(app2, live, peer, opcode, path, name));
        }
        log(&app, "TFTP 服务已停止".into());
    });
    Ok(true)
}

fn tftp_transfer(
    app: AppHandle,
    running: Arc<AtomicBool>,
    peer: SocketAddr,
    opcode: u16,
    path: PathBuf,
    name: String,
) {
    let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)) else {
        return;
    };
    socket.set_read_timeout(Some(Duration::from_secs(3))).ok();
    if opcode == 1 {
        let Ok(mut file) = std::fs::File::open(&path) else {
            let mut error = vec![0, 5, 0, 1];
            error.extend_from_slice(b"File not found\0");
            let _ = socket.send_to(&error, peer);
            return;
        };
        let mut block = 1_u16;
        loop {
            if !running.load(Ordering::Relaxed) {
                break;
            }
            let mut data = vec![0, 3];
            data.extend_from_slice(&block.to_be_bytes());
            let mut payload = [0_u8; 512];
            let Ok(n) = file.read(&mut payload) else {
                break;
            };
            data.extend_from_slice(&payload[..n]);
            let mut acked = false;
            for _ in 0..4 {
                let _ = socket.send_to(&data, peer);
                let mut ack = [0_u8; 32];
                if let Ok((m, from)) = socket.recv_from(&mut ack) {
                    if m >= 4 && from == peer
                        && ack[..4] == [0, 4, block.to_be_bytes()[0], block.to_be_bytes()[1]]
                    {
                        acked = true;
                        break;
                    }
                }
            }
            if !acked || n < 512 {
                break;
            }
            block = block.wrapping_add(1);
        }
        log(&app, format!("TFTP 下载 {peer} → {name}"));
    } else if opcode == 2 {
        let Ok(mut file) = std::fs::File::create(&path) else {
            return;
        };
        let _ = socket.send_to(&[0, 4, 0, 0], peer);
        let mut expected = 1_u16;
        loop {
            if !running.load(Ordering::Relaxed) {
                break;
            }
            let mut data = [0_u8; 516];
            let Ok((n, from)) = socket.recv_from(&mut data) else {
                break;
            };
            if from != peer || n < 4 || data[0..2] != [0, 3] {
                continue;
            }
            let block = u16::from_be_bytes([data[2], data[3]]);
            if block == expected {
                if file.write_all(&data[4..n]).is_err() {
                    break;
                }
                let _ = socket.send_to(&[0, 4, data[2], data[3]], peer);
                expected = expected.wrapping_add(1);
                if n < 516 {
                    break;
                }
            }
        }
        log(&app, format!("TFTP 上传 {peer} → {name}"));
    }
}

#[derive(Clone)]
struct DhcpConfig {
    server: Ipv4Addr,
    pool_start: Ipv4Addr,
    pool_end: Ipv4Addr,
    mask: Ipv4Addr,
    gateway: Ipv4Addr,
    dns: Ipv4Addr,
    lease_secs: u32,
}

fn dhcp_message_type(packet: &[u8]) -> Option<u8> {
    if packet.len() < 240 || packet[236..240] != [99, 130, 83, 99] {
        return None;
    }
    let mut pos = 240;
    while pos < packet.len() {
        let code = packet[pos];
        pos += 1;
        if code == 255 {
            break;
        }
        if code == 0 {
            continue;
        }
        let len = *packet.get(pos)? as usize;
        pos += 1;
        if pos + len > packet.len() {
            return None;
        }
        if code == 53 && len == 1 {
            return Some(packet[pos]);
        }
        pos += len;
    }
    None
}

fn choose_lease(cfg: &DhcpConfig, packet: &[u8]) -> Ipv4Addr {
    let start = u32::from(cfg.pool_start);
    let end = u32::from(cfg.pool_end).max(start);
    let span = end.saturating_sub(start).saturating_add(1);
    let hash = packet
        .get(28..34)
        .unwrap_or(&[])
        .iter()
        .fold(0_u32, |value, byte| {
            value.wrapping_mul(33).wrapping_add(*byte as u32)
        });
    Ipv4Addr::from(start.saturating_add(hash % span.max(1)))
}

fn push_dhcp_option(packet: &mut Vec<u8>, code: u8, value: &[u8]) {
    packet.extend([code, value.len() as u8]);
    packet.extend_from_slice(value);
}

fn build_dhcp_reply(
    request: &[u8],
    message_type: u8,
    cfg: &DhcpConfig,
    offered: Ipv4Addr,
) -> Option<Vec<u8>> {
    if request.len() < 240 {
        return None;
    }
    let mut reply = vec![0_u8; 240];
    reply[0] = 2;
    reply[1..4].copy_from_slice(&request[1..4]);
    reply[4..8].copy_from_slice(&request[4..8]);
    reply[16..20].copy_from_slice(&offered.octets());
    reply[20..24].copy_from_slice(&cfg.server.octets());
    reply[28..44].copy_from_slice(&request[28..44]);
    reply[236..240].copy_from_slice(&[99, 130, 83, 99]);
    push_dhcp_option(&mut reply, 53, &[message_type]);
    push_dhcp_option(&mut reply, 54, &cfg.server.octets());
    push_dhcp_option(&mut reply, 51, &cfg.lease_secs.to_be_bytes());
    push_dhcp_option(&mut reply, 1, &cfg.mask.octets());
    push_dhcp_option(&mut reply, 3, &cfg.gateway.octets());
    push_dhcp_option(&mut reply, 6, &cfg.dns.octets());
    reply.push(255);
    Some(reply)
}

fn start_dhcp(
    app: AppHandle,
    state: &NetServiceState,
    host: &str,
    port: u16,
    cfg: DhcpConfig,
) -> Result<bool, String> {
    let socket = UdpSocket::bind((host, port)).map_err(|e| format!("DHCP 绑定失败：{e}"))?;
    socket.set_broadcast(true).ok();
    socket
        .set_read_timeout(Some(Duration::from_millis(250)))
        .ok();
    let running = Arc::new(AtomicBool::new(true));
    let count = Arc::new(AtomicU64::new(0));
    install(state, "dhcp", format!("{host}:{port}"), running.clone(), count.clone())?;
    let leases = state.dhcp_leases.clone();
    if let Ok(mut current) = leases.lock() {
        current.clear();
    }
    std::thread::spawn(move || {
        log(&app, "DHCP 服务已启动".into());
        let mut buf = [0_u8; 1500];
        while running.load(Ordering::Relaxed) {
            let Ok((n, peer)) = socket.recv_from(&mut buf) else {
                continue;
            };
            let Some(kind) = dhcp_message_type(&buf[..n]) else {
                continue;
            };
            count.fetch_add(1, Ordering::Relaxed);
            if kind != 1 && kind != 3 {
                continue;
            }
            let offered = choose_lease(&cfg, &buf[..n]);
            let Some(reply) =
                build_dhcp_reply(&buf[..n], if kind == 1 { 2 } else { 5 }, &cfg, offered)
            else {
                continue;
            };
            let target = if peer.ip().is_unspecified() {
                SocketAddr::from(([255, 255, 255, 255], 68))
            } else {
                SocketAddr::new(peer.ip(), 68)
            };
            let _ = socket
                .send_to(&reply, target)
                .or_else(|_| socket.send_to(&reply, SocketAddr::from(([255, 255, 255, 255], 68))));
            let mac = buf
                .get(28..34)
                .map(|m| {
                    m.iter()
                        .map(|b| format!("{b:02X}"))
                        .collect::<Vec<_>>()
                        .join(":")
                })
                .unwrap_or_default();
            let action = if kind == 1 { "Offer" } else { "ACK" };
            if let Ok(mut current) = leases.lock() {
                current.retain(|lease| lease["mac"].as_str() != Some(mac.as_str()));
                current.push(json!({"mac":mac,"ip":offered.to_string(),"expire_secs":cfg.lease_secs}));
            }
            log(&app, format!("DHCP {action} {offered} → {mac}"));
        }
        log(&app, "DHCP 服务已停止".into());
    });
    Ok(true)
}

fn parse_users(text: &str) -> HashMap<String, String> {
    text.split([',', ';', '\n', '\r'])
        .filter_map(|line| {
            let (user, pass) = line.split_once([':', '='])?;
            Some((user.trim().to_string(), pass.trim().to_string()))
        })
        .collect()
}

fn radius_password(packet: &[u8], secret: &str, encrypted: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut previous: &[u8] = packet.get(4..20).unwrap_or(&[]);
    for chunk in encrypted.chunks(16) {
        let mut seed = secret.as_bytes().to_vec();
        seed.extend_from_slice(previous);
        let digest = md5::compute(seed);
        let plain = chunk
            .iter()
            .zip(digest.0)
            .map(|(a, b)| a ^ b)
            .collect::<Vec<_>>();
        output.extend_from_slice(&plain);
        previous = chunk;
    }
    while output.last() == Some(&0) {
        output.pop();
    }
    output
}

fn start_radius(
    app: AppHandle,
    state: &NetServiceState,
    host: &str,
    port: u16,
    secret: String,
    users: String,
) -> Result<bool, String> {
    let socket = UdpSocket::bind((host, port)).map_err(|e| format!("RADIUS 绑定失败：{e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(250)))
        .ok();
    let accounts = parse_users(&users);
    let running = Arc::new(AtomicBool::new(true));
    let count = Arc::new(AtomicU64::new(0));
    install(state, "radius", format!("{host}:{port}"), running.clone(), count.clone())?;
    state.radius_accept.store(0, Ordering::Relaxed);
    state.radius_reject.store(0, Ordering::Relaxed);
    state.radius_users.store(accounts.len() as u64, Ordering::Relaxed);
    let accepts = state.radius_accept.clone();
    let rejects = state.radius_reject.clone();
    std::thread::spawn(move || {
        log(
            &app,
            format!("RADIUS 服务已启动，已加载 {} 个用户", accounts.len()),
        );
        let mut buf = [0_u8; 4096];
        while running.load(Ordering::Relaxed) {
            let Ok((n, peer)) = socket.recv_from(&mut buf) else {
                continue;
            };
            if n < 20 || buf[0] != 1 {
                continue;
            }
            count.fetch_add(1, Ordering::Relaxed);
            let mut user = Vec::new();
            let mut encrypted = Vec::new();
            let mut pos = 20;
            while pos + 2 <= n {
                let len = buf[pos + 1] as usize;
                if len < 2 || pos + len > n {
                    break;
                }
                match buf[pos] {
                    1 => user = buf[pos + 2..pos + len].to_vec(),
                    2 => encrypted = buf[pos + 2..pos + len].to_vec(),
                    _ => {}
                }
                pos += len;
            }
            let username = String::from_utf8_lossy(&user).into_owned();
            let password =
                String::from_utf8_lossy(&radius_password(&buf[..n], &secret, &encrypted))
                    .into_owned();
            let accept = accounts
                .get(&username)
                .is_some_and(|expected| expected == &password);
            if accept {
                accepts.fetch_add(1, Ordering::Relaxed);
            } else {
                rejects.fetch_add(1, Ordering::Relaxed);
            }
            let mut reply = vec![if accept { 2 } else { 3 }, buf[1], 0, 20];
            reply.extend_from_slice(&[0_u8; 16]);
            let mut auth = reply[..4].to_vec();
            auth.extend_from_slice(&buf[4..20]);
            auth.extend_from_slice(secret.as_bytes());
            reply[4..20].copy_from_slice(&md5::compute(auth).0);
            let _ = socket.send_to(&reply, peer);
            log(
                &app,
                format!(
                    "RADIUS {} {username} ← {peer}",
                    if accept {
                        "Access-Accept"
                    } else {
                        "Access-Reject"
                    }
                ),
            );
        }
        log(&app, "RADIUS 服务已停止".into());
    });
    Ok(true)
}

fn accept_data(listener: &TcpListener) -> Result<std::net::TcpStream, String> {
    listener.set_nonblocking(true).ok();
    let started = std::time::Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if started.elapsed() > Duration::from_secs(10) {
                    return Err("FTP 数据连接超时".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn handle_ftp(mut control: std::net::TcpStream, root: &std::path::Path) {
    let _ = control.set_read_timeout(Some(Duration::from_secs(120)));
    let Ok(reader_stream) = control.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);
    let mut passive: Option<TcpListener> = None;
    let mut cwd = PathBuf::new();
    let _ = control.write_all(b"220 Network Toolbox FTP ready\r\n");
    loop {
        let mut line = String::new();
        if reader
            .read_line(&mut line)
            .ok()
            .filter(|n| *n > 0)
            .is_none()
        {
            break;
        }
        let (command, argument) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
        let command = command.to_ascii_uppercase();
        let reply = match command.as_str() {
            "USER" => "331 Password required\r\n".to_string(),
            "PASS" => "230 Logged on\r\n".to_string(),
            "SYST" => "215 UNIX Type: L8\r\n".to_string(),
            "TYPE" | "NOOP" => "200 OK\r\n".to_string(),
            "FEAT" => "211-Features\r\n UTF8\r\n SIZE\r\n PASV\r\n EPSV\r\n211 End\r\n".to_string(),
            "PWD" => format!("257 \"/{}\"\r\n", cwd.to_string_lossy().replace('\\', "/")),
            "CWD" => {
                let requested = if argument == "/" {
                    PathBuf::new()
                } else {
                    cwd.join(argument.trim_start_matches('/'))
                };
                match safe_path(root, requested.to_string_lossy().as_ref()) {
                    Ok(path) if path.is_dir() => {
                        cwd = requested;
                        "250 Directory changed\r\n".into()
                    }
                    _ => "550 Directory unavailable\r\n".into(),
                }
            }
            "CDUP" => {
                cwd.pop();
                "250 Directory changed\r\n".into()
            }
            "PASV" | "EPSV" => match TcpListener::bind(("0.0.0.0", 0)) {
                Ok(listener) => {
                    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
                    passive = Some(listener);
                    if command == "EPSV" {
                        format!("229 Entering Extended Passive Mode (|||{port}|)\r\n")
                    } else {
                        let ip = control
                            .local_addr()
                            .ok()
                            .and_then(|a| match a.ip() {
                                std::net::IpAddr::V4(v) => Some(v.octets()),
                                _ => None,
                            })
                            .unwrap_or([127, 0, 0, 1]);
                        format!(
                            "227 Entering Passive Mode ({},{},{},{},{},{})\r\n",
                            ip[0],
                            ip[1],
                            ip[2],
                            ip[3],
                            port / 256,
                            port % 256
                        )
                    }
                }
                Err(_) => "425 Cannot open passive connection\r\n".into(),
            },
            "SIZE" => safe_path(root, cwd.join(argument).to_string_lossy().as_ref())
                .ok()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| format!("213 {}\r\n", m.len()))
                .unwrap_or_else(|| "550 File unavailable\r\n".into()),
            "LIST" | "NLST" | "RETR" | "STOR" => {
                let Some(listener) = passive.take() else {
                    let _ = control.write_all(b"425 Use PASV first\r\n");
                    continue;
                };
                let _ = control.write_all(b"150 Opening data connection\r\n");
                match accept_data(&listener) {
                    Ok(mut data) => {
                        let requested = if argument.is_empty() {
                            cwd.clone()
                        } else {
                            cwd.join(argument)
                        };
                        let path = safe_path(root, requested.to_string_lossy().as_ref()).ok();
                        let transfer = if command == "RETR" {
                            path.and_then(|p| std::fs::File::open(p).ok())
                                .map(|mut f| std::io::copy(&mut f, &mut data).is_ok())
                                .unwrap_or(false)
                        } else if command == "STOR" {
                            path.and_then(|p| std::fs::File::create(p).ok())
                                .map(|mut f| std::io::copy(&mut data, &mut f).is_ok())
                                .unwrap_or(false)
                        } else {
                            let listing = path
                                .and_then(|p| std::fs::read_dir(p).ok())
                                .into_iter()
                                .flatten()
                                .filter_map(|e| e.ok())
                                .map(|e| {
                                    if command == "NLST" {
                                        format!("{}\r\n", e.file_name().to_string_lossy())
                                    } else {
                                        format!(
                                            "-rw-r--r-- 1 owner group {:>10} Jan 01 00:00 {}\r\n",
                                            e.metadata().map(|m| m.len()).unwrap_or(0),
                                            e.file_name().to_string_lossy()
                                        )
                                    }
                                })
                                .collect::<String>();
                            data.write_all(listing.as_bytes()).is_ok()
                        };
                        if transfer {
                            "226 Transfer complete\r\n".into()
                        } else {
                            "550 Transfer failed\r\n".into()
                        }
                    }
                    Err(_) => "425 Data connection failed\r\n".into(),
                }
            }
            "QUIT" => {
                let _ = control.write_all(b"221 Goodbye\r\n");
                break;
            }
            _ => "502 Command not implemented\r\n".to_string(),
        };
        let _ = control.write_all(reply.as_bytes());
    }
}
fn start_tcp(
    app: AppHandle,
    state: &NetServiceState,
    kind: &str,
    host: &str,
    port: u16,
    root: PathBuf,
) -> Result<bool, String> {
    let listener = TcpListener::bind((host, port))
        .map_err(|e| format!("{kind} 绑定 {host}:{port} 失败：{e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let running = Arc::new(AtomicBool::new(true));
    let count = Arc::new(AtomicU64::new(0));
    install(state, kind, format!("{host}:{port}"), running.clone(), count.clone())?;
    let label = kind.to_string();
    std::thread::spawn(move || {
        log(&app, format!("{label} 服务已启动，目录 {}", root.display()));
        while running.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, peer)) => {
                    count.fetch_add(1, Ordering::Relaxed);
                    let mut buf = [0u8; 8192];
                    if label == "http" {
                        if let Ok(n) = stream.read(&mut buf) {
                            let header_end = buf[..n]
                                .windows(4)
                                .position(|w| w == b"\r\n\r\n")
                                .map(|p| p + 4)
                                .unwrap_or(n);
                            let request = String::from_utf8_lossy(&buf[..header_end]);
                            let mut first = request.lines().next().unwrap_or("").split_whitespace();
                            let method = first.next().unwrap_or("GET");
                            let requested = first
                                .next()
                                .unwrap_or("/")
                                .split('?')
                                .next()
                                .unwrap_or("/")
                                .trim_start_matches('/');
                            let file = safe_path(&root, requested).ok();
                            let (status, body, ctype) = if method == "PUT" || method == "POST" {
                                if let Some(file) = file {
                                    let length = request
                                        .lines()
                                        .find_map(|line| {
                                            line.split_once(':')
                                                .filter(|(k, _)| {
                                                    k.eq_ignore_ascii_case("content-length")
                                                })
                                                .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                                        })
                                        .unwrap_or(n.saturating_sub(header_end));
                                    let mut body = buf[header_end..n].to_vec();
                                    while body.len() < length {
                                        let mut more = [0_u8; 8192];
                                        match stream.read(&mut more) {
                                            Ok(0) | Err(_) => break,
                                            Ok(size) => body.extend_from_slice(&more[..size]),
                                        }
                                    }
                                    body.truncate(length);
                                    if let Some(parent) = file.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    match std::fs::write(file, body) {
                                        Ok(_) => (
                                            "201 Created",
                                            b"uploaded".to_vec(),
                                            "text/plain; charset=utf-8",
                                        ),
                                        Err(_) => (
                                            "500 Internal Server Error",
                                            b"write failed".to_vec(),
                                            "text/plain; charset=utf-8",
                                        ),
                                    }
                                } else {
                                    (
                                        "403 Forbidden",
                                        b"forbidden".to_vec(),
                                        "text/plain; charset=utf-8",
                                    )
                                }
                            } else if file.as_ref().is_some_and(|path| path.is_file()) {
                                (
                                    "200 OK",
                                    std::fs::read(file.unwrap()).unwrap_or_default(),
                                    "application/octet-stream",
                                )
                            } else if let Some(directory) = file.filter(|path| path.is_dir()) {
                                let listing = std::fs::read_dir(directory)
                                    .ok()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|e| e.ok())
                                    .map(|e| {
                                        format!(
                                            "<li><a href=\"{}\">{}</a></li>",
                                            e.file_name().to_string_lossy(),
                                            e.file_name().to_string_lossy()
                                        )
                                    })
                                    .collect::<String>();
                                ("200 OK",format!("<!doctype html><meta charset=\"utf-8\"><h1>Network Toolbox</h1><ul>{listing}</ul>").into_bytes(),"text/html; charset=utf-8")
                            } else {
                                (
                                    "404 Not Found",
                                    b"not found".to_vec(),
                                    "text/plain; charset=utf-8",
                                )
                            };
                            let _=write!(stream,"HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {ctype}\r\nConnection: close\r\n\r\n",body.len());
                            if method != "HEAD" {
                                let _ = stream.write_all(&body);
                            }
                        }
                    } else {
                        let ftp_root = root.clone();
                        std::thread::spawn(move || handle_ftp(stream, &ftp_root));
                    }
                    log(&app, format!("{label} 接入 {peer}"));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                Err(_) => break,
            }
        }
        log(&app, format!("{label} 服务已停止"));
    });
    Ok(true)
}
fn stop(state: &NetServiceState, kind: &str) -> Result<bool, String> {
    if let Some(service) = state
        .services
        .lock()
        .map_err(|_| "服务状态锁异常")?
        .remove(kind)
    {
        service.running.store(false, Ordering::Relaxed);
    }
    Ok(true)
}

#[tauri::command]
pub fn net_svc_status(state: State<'_, NetServiceState>) -> Result<Value, String> {
    let map = state.services.lock().map_err(|_| "服务状态锁异常")?;
    let addr = |k: &str| map.get(k).map(|s| s.address.clone());
    let running = |k: &str| map.get(k).is_some_and(|s| s.running.load(Ordering::Relaxed));
    let count = |k: &str| map.get(k).map_or(0, |s| s.count.load(Ordering::Relaxed));
    let leases = state.dhcp_leases.lock().map_err(|_| "DHCP 租约锁异常")?.clone();
    Ok(json!({
        "syslog_running":running("syslog"), "http_running":running("http"),
        "tftp_running":running("tftp"), "ftp_running":running("ftp"),
        "dhcp_running":running("dhcp"), "radius_running":running("radius"),
        "syslog_count":count("syslog"), "http_count":count("http"),
        "tftp_count":count("tftp"), "ftp_count":count("ftp"),
        "dhcp_count":count("dhcp"), "radius_count":count("radius"),
        "syslog_addr":addr("syslog"), "http_addr":addr("http"),
        "tftp_addr":addr("tftp"), "ftp_addr":addr("ftp"),
        "dhcp_addr":addr("dhcp"), "radius_addr":addr("radius"),
        "dhcp_leases":leases,
        "radius_accept":state.radius_accept.load(Ordering::Relaxed),
        "radius_reject":state.radius_reject.load(Ordering::Relaxed),
        "radius_users":state.radius_users.load(Ordering::Relaxed)
    }))
}
#[tauri::command]
pub fn net_svc_start_syslog(
    app: AppHandle,
    state: State<'_, NetServiceState>,
    host: String,
    port: u16,
) -> Result<bool, String> {
    start_udp(app, &state, "syslog", &host, port)
}
#[tauri::command]
pub fn net_svc_start_tftp(
    app: AppHandle,
    state: State<'_, NetServiceState>,
    host: String,
    port: u16,
    root: String,
) -> Result<bool, String> {
    start_tftp(app, &state, &host, port, service_root(&root))
}
#[tauri::command]
pub fn net_svc_start_radius(
    app: AppHandle,
    state: State<'_, NetServiceState>,
    host: String,
    port: u16,
    secret: String,
    users: String,
) -> Result<bool, String> {
    start_radius(app, &state, &host, port, secret, users)
}
#[tauri::command]
pub fn net_svc_start_dhcp(
    app: AppHandle,
    state: State<'_, NetServiceState>,
    host: String,
    port: u16,
    server_ip: String,
    pool_start: String,
    pool_end: String,
    mask: String,
    gateway: String,
    dns: String,
    lease_secs: u32,
) -> Result<bool, String> {
    let parse = |value: &str, label: &str| {
        value
            .parse::<Ipv4Addr>()
            .map_err(|_| format!("{label} IPv4 地址无效：{value}"))
    };
    let cfg = DhcpConfig {
        server: parse(&server_ip, "服务器")?,
        pool_start: parse(&pool_start, "地址池起点")?,
        pool_end: parse(&pool_end, "地址池终点")?,
        mask: parse(&mask, "子网掩码")?,
        gateway: parse(&gateway, "网关")?,
        dns: parse(dns.split(',').next().unwrap_or(&dns).trim(), "DNS")?,
        lease_secs: lease_secs.clamp(60, 31_536_000),
    };
    if u32::from(cfg.pool_start) > u32::from(cfg.pool_end) {
        return Err("DHCP 地址池起点不能大于终点".into());
    }
    start_dhcp(app, &state, &host, port, cfg)
}
#[tauri::command]
pub fn net_svc_start_http(
    app: AppHandle,
    state: State<'_, NetServiceState>,
    host: String,
    port: u16,
    root: String,
) -> Result<bool, String> {
    let path = service_root(&root);
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    start_tcp(app, &state, "http", &host, port, path)
}
#[tauri::command]
pub fn net_svc_start_ftp(
    app: AppHandle,
    state: State<'_, NetServiceState>,
    host: String,
    port: u16,
    root: String,
) -> Result<bool, String> {
    let path = service_root(&root);
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    start_tcp(app, &state, "ftp", &host, port, path)
}
macro_rules! stops{($($fn:ident=>$kind:literal),*)=>{$(#[tauri::command]pub fn $fn(state:State<'_,NetServiceState>)->Result<bool,String>{stop(&state,$kind)})*}}
stops!(net_svc_stop_syslog=>"syslog",net_svc_stop_http=>"http",net_svc_stop_tftp=>"tftp",net_svc_stop_ftp=>"ftp",net_svc_stop_dhcp=>"dhcp",net_svc_stop_radius=>"radius");
#[tauri::command]
pub fn net_svc_smoke_test() -> Value {
    let tests = [
        ("TCP 服务监听", TcpListener::bind(("127.0.0.1", 0)).is_ok(), "HTTP/FTP 可创建本机监听端口"),
        ("UDP 服务监听", UdpSocket::bind(("127.0.0.1", 0)).is_ok(), "TFTP/Syslog/DHCP/RADIUS 可创建本机监听端口"),
        ("服务目录", std::fs::metadata(std::env::temp_dir()).is_ok(), "服务根目录可访问"),
    ];
    let ok = tests.iter().all(|test| test.1);
    let cases = tests.into_iter().map(|(name,ok,detail)|json!({"name":name,"ok":ok,"detail":detail})).collect::<Vec<_>>();
    json!({"ok":ok,"message":if ok{"网络服务运行环境自检通过"}else{"网络服务环境存在异常"},"cases":cases})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_path_escape() {
        assert!(safe_path(std::path::Path::new("C:\\service"), "../secret").is_err());
        assert!(safe_path(std::path::Path::new("C:\\service"), "folder/file.txt").is_ok());
    }

    #[test]
    fn builds_valid_dhcp_offer() {
        let mut request = vec![0_u8; 244];
        request[0] = 1;
        request[1] = 1;
        request[2] = 6;
        request[4..8].copy_from_slice(&[1, 2, 3, 4]);
        request[28..34].copy_from_slice(&[0, 1, 2, 3, 4, 5]);
        request[236..240].copy_from_slice(&[99, 130, 83, 99]);
        request[240..244].copy_from_slice(&[53, 1, 1, 255]);
        let cfg = DhcpConfig {
            server: "192.168.1.1".parse().unwrap(),
            pool_start: "192.168.1.100".parse().unwrap(),
            pool_end: "192.168.1.200".parse().unwrap(),
            mask: "255.255.255.0".parse().unwrap(),
            gateway: "192.168.1.1".parse().unwrap(),
            dns: "223.5.5.5".parse().unwrap(),
            lease_secs: 3600,
        };
        assert_eq!(dhcp_message_type(&request), Some(1));
        let offered = choose_lease(&cfg, &request);
        let reply = build_dhcp_reply(&request, 2, &cfg, offered).unwrap();
        assert_eq!(reply[0], 2);
        assert_eq!(&reply[4..8], &[1, 2, 3, 4]);
        assert_eq!(&reply[236..240], &[99, 130, 83, 99]);
        assert_eq!(dhcp_message_type(&reply), Some(2));
    }

    #[test]
    fn parses_radius_accounts() {
        let users = parse_users("admin:secret\noperator = pass123\ninvalid");
        assert_eq!(users.get("admin").map(String::as_str), Some("secret"));
        assert_eq!(users.get("operator").map(String::as_str), Some("pass123"));
        assert_eq!(users.len(), 2);
    }
}
