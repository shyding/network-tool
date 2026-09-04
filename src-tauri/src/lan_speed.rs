use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
#[derive(Default)]
pub struct LanSpeedState {
    server: Arc<AtomicBool>,
    client: Arc<AtomicBool>,
}
fn start_udp_server(
    app: AppHandle,
    running: Arc<AtomicBool>,
    bind_ip: String,
    port: u16,
) -> Result<(), String> {
    let socket = std::net::UdpSocket::bind((bind_ip.as_str(), port))
        .map_err(|e| format!("UDP 测速服务端绑定失败：{e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(250)))
        .ok();
    std::thread::spawn(move || {
        let _ = app.emit(
            "lanspeed:server_log",
            json!({"side":"server","message":format!("UDP 服务端已启动 {bind_ip}:{port}")}),
        );
        let mut buf = [0_u8; 65_535];
        while running.load(Ordering::Relaxed) {
            let Ok((n, peer)) = socket.recv_from(&mut buf) else {
                continue;
            };
            if n >= 3 && buf[0] == b'D' {
                let seconds = u16::from_be_bytes([buf[1], buf[2]]).clamp(1, 300) as u64;
                let Ok(sender) = socket.try_clone() else {
                    continue;
                };
                let live = running.clone();
                let app2 = app.clone();
                std::thread::spawn(move || {
                    let payload = vec![b'D'; 1200];
                    let started = Instant::now();
                    let mut bytes = 0_u64;
                    while live.load(Ordering::Relaxed)
                        && started.elapsed() < Duration::from_secs(seconds)
                    {
                        if sender.send_to(&payload, peer).is_err() {
                            break;
                        }
                        bytes += payload.len() as u64;
                    }
                    let _ = app2.emit("lanspeed:server_log", json!({"side":"server","message":format!("UDP 下载流 {peer} 发送 {bytes} 字节")}));
                });
            }
        }
        let _ = app.emit(
            "lanspeed:server_log",
            json!({"side":"server","message":"UDP 服务端已停止"}),
        );
    });
    Ok(())
}
#[tauri::command]
pub fn check_iperf3() -> Value {
    let out = Command::new("where.exe")
        .arg("iperf3.exe")
        .creation_flags(0x08000000)
        .output();
    let installed = out.as_ref().is_ok_and(|o| o.status.success());
    let path = out
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(str::to_string));
    json!({"installed":installed,"available":installed,"path":path,"version":if installed{"已安装"}else{""},"message":if installed{"iPerf3 已就绪"}else{"未找到 iperf3.exe，可使用内置 TCP 引擎"}})
}
#[tauri::command]
pub fn start_lan_server(
    app: AppHandle,
    state: State<'_, LanSpeedState>,
    bind_ip: String,
    port: u16,
    engine: String,
) -> Result<bool, String> {
    state.server.store(false, Ordering::Relaxed);
    let running = state.server.clone();
    running.store(true, Ordering::Relaxed);
    if engine == "udp" {
        if let Err(error) = start_udp_server(app, running.clone(), bind_ip, port) {
            running.store(false, Ordering::Relaxed);
            return Err(error);
        }
        return Ok(true);
    }
    if engine == "iperf3" {
        let mut child = Command::new("iperf3.exe")
            .args(["-s", "-B", &bind_ip, "-p", &port.to_string()])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| format!("启动 iPerf3 服务端失败：{e}"))?;
        std::thread::spawn(move || {
            let _ = app.emit(
                "lanspeed:server_log",
                json!({"side":"server","message":format!("iPerf3 服务端已启动 {bind_ip}:{port}")}),
            );
            while running.load(Ordering::Relaxed) {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            let _ = child.kill();
            let _ = child.wait();
            let _ = app.emit(
                "lanspeed:server_log",
                json!({"side":"server","message":"iPerf3 服务端已停止"}),
            );
        });
        return Ok(true);
    }
    if engine != "builtin" {
        running.store(false, Ordering::Relaxed);
        return Err(format!("未知测速引擎：{engine}"));
    }
    let listener = TcpListener::bind((bind_ip.as_str(), port))
        .map_err(|e| format!("测速服务端绑定失败：{e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = app.emit(
            "lanspeed:server_log",
            json!({"side":"server","message":format!("服务端已启动 {bind_ip}:{port}")}),
        );
        while running.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, peer)) => {
                    let live = running.clone();
                    let app2 = app.clone();
                    std::thread::spawn(move || {
                        let mut mode = [0u8; 1];
                        if stream.read_exact(&mut mode).is_err() {
                            return;
                        }
                        let mut buf = vec![0u8; 256 * 1024];
                        let mut bytes = 0u64;
                        let started = Instant::now();
                        if mode[0] == b'U' {
                            while live.load(Ordering::Relaxed) {
                                match stream.read(&mut buf) {
                                    Ok(0) | Err(_) => break,
                                    Ok(n) => bytes += n as u64,
                                }
                            }
                        } else {
                            while live.load(Ordering::Relaxed) {
                                if stream.write_all(&buf).is_err() {
                                    break;
                                }
                                bytes += buf.len() as u64;
                            }
                        }
                        let _=app2.emit("lanspeed:server_log",json!({"side":"server","message":format!("{peer} 传输 {} 字节，{:.1}s",bytes,started.elapsed().as_secs_f64())}));
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                Err(_) => break,
            }
        }
        let _ = app.emit(
            "lanspeed:server_log",
            json!({"side":"server","message":"服务端已停止"}),
        );
    });
    Ok(true)
}
#[tauri::command]
pub fn stop_lan_server(state: State<'_, LanSpeedState>) -> bool {
    state.server.store(false, Ordering::Relaxed);
    true
}
fn direction(
    app: &AppHandle,
    running: &AtomicBool,
    target: &str,
    mode: u8,
    duration: u64,
    label: &str,
) -> Result<f64, String> {
    let mut stream = TcpStream::connect_timeout(
        &target.parse().map_err(|_| "服务端地址无效")?,
        Duration::from_secs(5),
    )
    .map_err(|e| format!("连接测速服务端失败：{e}"))?;
    stream.write_all(&[mode]).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    let started = Instant::now();
    let mut bytes = 0u64;
    let mut last = Instant::now();
    while running.load(Ordering::Relaxed)
        && started.elapsed() < Duration::from_secs(duration.clamp(1, 300))
    {
        let n = if mode == b'U' {
            stream.write_all(&buf).map(|_| buf.len())
        } else {
            stream.read(&mut buf)
        }
        .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if last.elapsed() >= Duration::from_secs(1) {
            let mbps = bytes as f64 * 8.0 / started.elapsed().as_secs_f64() / 1_000_000.0;
            let _ = app.emit(
                "lanspeed:sample",
                json!({"direction":label,"t":started.elapsed().as_secs_f64(),"mbps":mbps}),
            );
            let _ = app.emit(
                "lanspeed:progress",
                json!({"side":"client","message":format!("{label} {mbps:.1} Mbps"),"mbps":mbps}),
            );
            last = Instant::now();
        }
    }
    Ok(bytes as f64 * 8.0 / started.elapsed().as_secs_f64().max(0.001) / 1_000_000.0)
}

fn udp_direction(
    app: &AppHandle,
    running: &AtomicBool,
    target: &str,
    duration: u64,
    upload: bool,
) -> Result<f64, String> {
    let socket = std::net::UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| e.to_string())?;
    socket
        .connect(target)
        .map_err(|e| format!("连接 UDP 测速服务端失败：{e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(300)))
        .ok();
    let duration = duration.clamp(1, 300);
    let started = Instant::now();
    let mut bytes = 0_u64;
    let mut last = Instant::now();
    let payload = vec![b'U'; 1200];
    if !upload {
        let mut request = vec![b'D'];
        request.extend_from_slice(&(duration as u16).to_be_bytes());
        socket.send(&request).map_err(|e| e.to_string())?;
    }
    let mut buf = [0_u8; 65_535];
    while running.load(Ordering::Relaxed) && started.elapsed() < Duration::from_secs(duration) {
        let amount = if upload {
            socket.send(&payload).map_err(|e| e.to_string())?
        } else {
            match socket.recv(&mut buf) {
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    continue
                }
                Err(e) => return Err(e.to_string()),
            }
        };
        bytes += amount as u64;
        if last.elapsed() >= Duration::from_secs(1) {
            let mbps = bytes as f64 * 8.0 / started.elapsed().as_secs_f64() / 1_000_000.0;
            let label = if upload { "upload" } else { "download" };
            let _ = app.emit(
                "lanspeed:sample",
                json!({"direction":label,"t":started.elapsed().as_secs_f64(),"mbps":mbps}),
            );
            let _ = app.emit("lanspeed:progress", json!({"side":"client","message":format!("UDP {label} {mbps:.1} Mbps"),"mbps":mbps}));
            last = Instant::now();
        }
    }
    Ok(bytes as f64 * 8.0 / started.elapsed().as_secs_f64().max(0.001) / 1_000_000.0)
}

fn iperf_direction(
    server: &str,
    port: u16,
    duration: u64,
    upload: bool,
    udp: bool,
) -> Result<f64, String> {
    let mut args = vec![
        "-c".to_string(),
        server.to_string(),
        "-p".to_string(),
        port.to_string(),
        "-t".to_string(),
        duration.clamp(1, 300).to_string(),
        "-J".to_string(),
    ];
    if !upload {
        args.push("-R".into());
    }
    if udp {
        args.extend(["-u".into(), "-b".into(), "0".into()]);
    }
    let output = Command::new("iperf3.exe")
        .args(&args)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("执行 iPerf3 失败：{e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let value: Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("解析 iPerf3 结果失败：{e}"))?;
    let bps = value
        .pointer("/end/sum_received/bits_per_second")
        .or_else(|| value.pointer("/end/sum_sent/bits_per_second"))
        .and_then(Value::as_f64)
        .ok_or("iPerf3 结果缺少速率")?;
    Ok(bps / 1_000_000.0)
}
#[tauri::command]
pub fn start_lan_speed_test(
    app: AppHandle,
    state: State<'_, LanSpeedState>,
    server_ip: String,
    port: u16,
    duration: u64,
    mode: String,
    engine: String,
    udp: bool,
) -> Result<bool, String> {
    if !matches!(engine.as_str(), "builtin" | "udp" | "iperf3") {
        return Err(format!("未知测速引擎：{engine}"));
    }
    state.client.store(false, Ordering::Relaxed);
    let running = state.client.clone();
    running.store(true, Ordering::Relaxed);
    std::thread::spawn(move || {
        let target = format!("{server_ip}:{port}");
        let started = Instant::now();
        let mut up = 0.0;
        let mut down = 0.0;
        let result = (|| -> Result<(), String> {
            if mode == "upload" || mode == "both" {
                up = match engine.as_str() {
                    "builtin" => direction(&app, &running, &target, b'U', duration, "upload")?,
                    "udp" => udp_direction(&app, &running, &target, duration, true)?,
                    "iperf3" => iperf_direction(&server_ip, port, duration, true, udp)?,
                    _ => unreachable!(),
                };
            }
            if mode == "download" || mode == "both" {
                down = match engine.as_str() {
                    "builtin" => direction(&app, &running, &target, b'D', duration, "download")?,
                    "udp" => udp_direction(&app, &running, &target, duration, false)?,
                    "iperf3" => iperf_direction(&server_ip, port, duration, false, udp)?,
                    _ => unreachable!(),
                };
            }
            Ok(())
        })();
        running.store(false, Ordering::Relaxed);
        match result {
            Ok(_) => {
                let _=app.emit("lanspeed:complete",json!({"ok":true,"upload_mbps":up,"download_mbps":down,"elapsed_s":started.elapsed().as_secs_f64()}));
            }
            Err(e) => {
                let _ = app.emit(
                    "lanspeed:complete",
                    json!({"ok":false,"error":e,"upload_mbps":up,"download_mbps":down}),
                );
            }
        }
    });
    Ok(true)
}
#[tauri::command]
pub fn stop_lan_speed_test(state: State<'_, LanSpeedState>) -> bool {
    state.client.store(false, Ordering::Relaxed);
    true
}
