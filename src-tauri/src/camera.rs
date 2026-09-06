use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::net::UdpSocket;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct CameraState {
    scan_cancel: AtomicBool,
    streaming: std::sync::Arc<AtomicBool>,
    upgrade_cancel: AtomicBool,
}
#[tauri::command]
pub fn list_camera_vendors() -> Value {
    json!([
     {"id":"auto","label":"自动识别"},{"id":"hikvision","label":"海康威视"},{"id":"dahua","label":"大华"},{"id":"uniview","label":"宇视"},{"id":"xm","label":"雄迈/XM"},{"id":"onvif","label":"通用 ONVIF"}
    ])
}
fn rtsp(host: &str, user: &str, password: &str, brand: Option<&str>, channel: u32) -> String {
    let auth = if user.is_empty() {
        String::new()
    } else {
        format!("{}:{}@", user, password)
    };
    let path = match brand.unwrap_or("auto").to_ascii_lowercase().as_str() {
        "hikvision" | "海康威视" => format!("/Streaming/Channels/{}01", channel),
        "dahua" | "大华" => format!("/cam/realmonitor?channel={channel}&subtype=0"),
        "uniview" | "宇视" => format!("/media/video{channel}"),
        _ => format!("/live/ch{channel}"),
    };
    format!("rtsp://{auth}{host}:554{path}")
}
fn multicast_camera_discovery(mode: &str, timeout_ms: u64) -> Result<Vec<Value>, String> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| format!("组播发现绑定失败：{e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(250)))
        .ok();
    socket.set_multicast_loop_v4(false).ok();
    let (target, request) = if mode == "onvif" {
        (
            "239.255.255.250:3702",
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?><e:Envelope xmlns:e=\"http://www.w3.org/2003/05/soap-envelope\" xmlns:w=\"http://schemas.xmlsoap.org/ws/2004/08/addressing\" xmlns:d=\"http://schemas.xmlsoap.org/ws/2005/04/discovery\" xmlns:dn=\"http://www.onvif.org/ver10/network/wsdl\"><e:Header><w:MessageID>uuid:network-toolbox-{}</w:MessageID><w:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</w:To><w:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</w:Action></e:Header><e:Body><d:Probe><d:Types>dn:NetworkVideoTransmitter</d:Types></d:Probe></e:Body></e:Envelope>",
                std::process::id()
            ),
        )
    } else {
        (
            "239.255.255.250:1900",
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: ssdp:all\r\n\r\n".into(),
        )
    };
    socket
        .send_to(request.as_bytes(), target)
        .map_err(|e| format!("发送组播探测失败：{e}"))?;
    let started = Instant::now();
    let mut devices = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut buf = [0_u8; 65_535];
    while started.elapsed() < Duration::from_millis(timeout_ms.clamp(500, 15_000)) {
        let Ok((n, peer)) = socket.recv_from(&mut buf) else {
            continue;
        };
        if !seen.insert(peer.ip()) {
            continue;
        }
        let text = String::from_utf8_lossy(&buf[..n]);
        let location = text
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(key, _)| key.trim().eq_ignore_ascii_case("location"))
                    .map(|(_, value)| value.trim().to_string())
            })
            .or_else(|| {
                let start = text
                    .find("<d:XAddrs>")
                    .or_else(|| text.find("<wsdd:XAddrs>"))?;
                let tail = &text[start..];
                let value_start = tail.find('>')? + 1;
                let value_end = tail[value_start..].find('<')? + value_start;
                Some(
                    tail[value_start..value_end]
                        .split_whitespace()
                        .next()?
                        .to_string(),
                )
            });
        devices.push(json!({"ip":peer.ip().to_string(),"port":peer.port(),"http_port":location.as_deref().and_then(|url|url.split(':').nth(2)).and_then(|part|part.split('/').next()).and_then(|p|p.parse::<u16>().ok()),"rtsp_port":554,"ports":[peer.port()],"brand":"onvif","vendor":if mode=="onvif"{"ONVIF"}else{"SSDP"},"model":"待认证读取","name":format!("组播摄像头 {}",peer.ip()),"location":location,"discovery":mode}));
    }
    Ok(devices)
}
#[tauri::command]
pub fn camera_build_rtsp_url(
    host: String,
    username: String,
    password: String,
    brand: Option<String>,
    channel: u32,
) -> Result<String, String> {
    if host.trim().is_empty() {
        return Err("设备地址不能为空".into());
    }
    Ok(rtsp(
        &host,
        &username,
        &password,
        brand.as_deref(),
        channel.max(1),
    ))
}

#[tauri::command]
pub async fn start_camera_scan(
    app: AppHandle,
    state: State<'_, CameraState>,
    network: String,
    mode: String,
    threads: usize,
    timeout_ms: u64,
) -> Result<Value, String> {
    if mode == "ssdp" || mode == "onvif" {
        state.scan_cancel.store(false, Ordering::Relaxed);
        let devices = multicast_camera_discovery(&mode, timeout_ms)?;
        for device in &devices {
            let _ = app.emit("camera:device", device);
        }
        let result = json!({"network":network,"mode":mode,"devices":devices,"total":devices.len(),"scanned":devices.len(),"found":devices.len(),"message":format!("组播发现完成，找到 {} 台设备",devices.len())});
        let _ = app.emit("camera:progress", json!({"scanned":devices.len(),"total":devices.len(),"found":devices.len(),"percent":100}));
        let _ = app.emit("camera:complete", &result);
        return Ok(result);
    }
    let net: ipnet::Ipv4Net = network.parse().map_err(|_| "网络范围必须是 IPv4 CIDR")?;
    let addresses = net.hosts().take(65_536).collect::<Vec<_>>();
    state.scan_cancel.store(false, Ordering::Relaxed);
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(threads.clamp(1, 256)));
    let mut set = tokio::task::JoinSet::new();
    for ip in addresses.iter().copied() {
        let permit = sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
        set.spawn(async move {
            let _p = permit;
            let mut ports = Vec::new();
            for port in [80u16, 443, 554, 8000, 8080, 8899, 34567, 37777] {
                if matches!(
                    tokio::time::timeout(
                        Duration::from_millis(timeout_ms.clamp(100, 5000)),
                        tokio::net::TcpStream::connect((ip, port))
                    )
                    .await,
                    Ok(Ok(_))
                ) {
                    ports.push(port)
                }
            }
            (ip, ports)
        });
    }
    let started = Instant::now();
    let mut scanned = 0;
    let mut devices = Vec::new();
    while let Some(item) = set.join_next().await {
        if state.scan_cancel.load(Ordering::Relaxed) {
            set.abort_all();
            break;
        }
        let (ip, ports) = item.map_err(|e| e.to_string())?;
        scanned += 1;
        if ports.contains(&554)
            || ports.contains(&8000)
            || ports.contains(&37777)
            || ports.contains(&34567)
        {
            let brand = if ports.contains(&8000) {
                "hikvision"
            } else if ports.contains(&37777) {
                "dahua"
            } else if ports.contains(&34567) {
                "xm"
            } else {
                "onvif"
            };
            let device = json!({"ip":ip.to_string(),"port":ports.first().copied().unwrap_or(80),"http_port":ports.iter().find(|p|**p==80||**p==8080).copied(),"rtsp_port":ports.iter().find(|p|**p==554).copied(),"ports":ports,"brand":brand,"vendor":brand,"model":"未知","name":format!("摄像头 {}",ip)});
            let _ = app.emit("camera:device", &device);
            devices.push(device);
        }
        let _=app.emit("camera:progress",json!({"scanned":scanned,"total":addresses.len(),"found":devices.len(),"percent":scanned as f64*100.0/addresses.len().max(1)as f64}));
    }
    let result = json!({"network":network,"mode":mode,"devices":devices,"total":addresses.len(),"scanned":scanned,"found":devices.len(),"elapsed_ms":started.elapsed().as_millis()});
    let _ = app.emit("camera:complete", &result);
    Ok(result)
}
#[tauri::command]
pub fn stop_camera_scan(state: State<'_, CameraState>) -> bool {
    state.scan_cancel.store(true, Ordering::Relaxed);
    true
}

fn ffmpeg() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let local = exe
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("ffmpeg.exe");
    if local.exists() {
        Ok(local)
    } else {
        Ok("ffmpeg.exe".into())
    }
}
fn snapshot_bytes(url: &str, timeout: u64) -> Result<Vec<u8>, String> {
    let timeout_us = timeout.clamp(500, 120_000).saturating_mul(1000).to_string();
    let out = Command::new(ffmpeg()?)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-rtsp_transport",
            "tcp",
            "-rw_timeout",
            &timeout_us,
            "-i",
            url,
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "pipe:1",
        ])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("启动 ffmpeg 失败：{e}"))?;
    if out.status.success() && !out.stdout.is_empty() {
        Ok(out.stdout)
    } else {
        Err(format!(
            "截图失败：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
fn http_snapshot(
    url: &str,
    username: &str,
    password: &str,
    timeout_ms: u64,
) -> Result<Vec<u8>, String> {
    let output = Command::new("curl.exe")
        .args([
            "-sS",
            "-k",
            "--anyauth",
            "-u",
            &format!("{username}:{password}"),
            "--max-time",
            &(timeout_ms / 1000).max(1).to_string(),
            url,
        ])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() && output.stdout.len() > 128 {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
#[tauri::command(async)]
pub fn camera_get_snapshot(
    host: String,
    port: u16,
    username: String,
    password: String,
    brand: Option<String>,
    channel: u32,
    https: bool,
    timeout_ms: u64,
) -> Result<Value, String> {
    let scheme = if https { "https" } else { "http" };
    let brand_key = brand.as_deref().unwrap_or("auto").to_ascii_lowercase();
    let mut paths = Vec::new();
    if brand_key == "auto" || brand_key.contains("hik") || brand_key.contains("海康") {
        paths.push(format!(
            "/ISAPI/Streaming/channels/{}01/picture",
            channel.max(1)
        ));
    }
    if brand_key == "auto" || brand_key.contains("dahua") || brand_key.contains("大华") {
        paths.push(format!(
            "/cgi-bin/snapshot.cgi?channel={}",
            channel.saturating_sub(1)
        ));
    }
    if brand_key == "auto" || brand_key.contains("uniview") || brand_key.contains("宇视") {
        paths.push(format!(
            "/LAPI/V1.0/Channels/{}/Media/Video/Streams/0/Snapshot",
            channel.max(1)
        ));
    }
    if brand_key == "auto"
        || brand_key.contains("xm")
        || brand_key.contains("雄迈")
        || brand_key.contains("中维")
    {
        paths.push(format!(
            "/webcapture.jpg?command=snap&channel={}",
            channel.saturating_sub(1)
        ));
        paths.push("/snap.jpg".into());
        paths.push("/images_cgi/now.jpg".into());
    }
    for path in paths {
        let endpoint = format!("{scheme}://{host}:{port}{path}");
        if let Ok(bytes) = http_snapshot(&endpoint, &username, &password, timeout_ms) {
            return Ok(
                json!({"ok":true,"message":"HTTP 抓图成功","data_url":format!("data:image/jpeg;base64,{}",base64::engine::general_purpose::STANDARD.encode(&bytes)),"bytes":bytes.len(),"snapshot_url":endpoint,"protocol":scheme.to_uppercase()}),
            );
        }
    }
    let mut url = rtsp(&host, &username, &password, brand.as_deref(), channel);
    if port != 0 && port != 554 {
        url = url.replacen(&format!("{host}:554"), &format!("{host}:{port}"), 1);
    }
    match snapshot_bytes(&url, timeout_ms) {
        Ok(bytes) => Ok(
            json!({"ok":true,"message":"截图成功","data_url":format!("data:image/jpeg;base64,{}",base64::engine::general_purpose::STANDARD.encode(&bytes)),"bytes":bytes.len(),"rtsp_url":url,"protocol":"RTSP"}),
        ),
        Err(message) => Ok(
            json!({"ok":false,"message":message,"data_url":null,"bytes":0,"rtsp_url":url,"protocol":if https{"HTTPS/RTSP"}else{"HTTP/RTSP"}}),
        ),
    }
}
#[tauri::command(async)]
pub fn camera_start_live_stream(
    app: AppHandle,
    state: State<'_, CameraState>,
    host: String,
    username: String,
    password: String,
    brand: Option<String>,
    channel: u32,
) -> Result<Value, String> {
    state.streaming.store(false, Ordering::Relaxed);
    let running = state.streaming.clone();
    running.store(true, Ordering::Relaxed);
    let url = rtsp(&host, &username, &password, brand.as_deref(), channel);
    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            match snapshot_bytes(&url, 6000) {
                Ok(bytes) => {
                    let _ = app.emit(
                        "camera:stream-frame",
                        format!(
                            "data:image/jpeg;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(bytes)
                        ),
                    );
                    let _ = app.emit(
                        "camera:stream-status",
                        json!({"running":true,"message":"实时预览中"}),
                    );
                }
                Err(e) => {
                    let _ = app.emit("camera:stream-status", json!({"running":false,"message":e}));
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(2500));
        }
        running.store(false, Ordering::Relaxed);
    });
    Ok(json!({"running":true,"message":"正在启动实时预览"}))
}
#[tauri::command(async)]
pub fn camera_stop_live_stream(state: State<'_, CameraState>) -> Value {
    state.streaming.store(false, Ordering::Relaxed);
    json!({"running":false,"message":"实时预览已停止"})
}
#[tauri::command(async)]
pub fn camera_inspect_firmware(path: String) -> Result<Value, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("读取固件失败：{e}"))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let file = std::path::Path::new(&path);
    Ok(
        json!({"path":path,"file_name":file.file_name().and_then(|s|s.to_str()).unwrap_or("firmware"),"bytes":bytes.len(),"sha256":hash,"supported_hint":"已完成文件完整性识别；升级前必须人工核对型号和硬件版本"}),
    )
}
#[tauri::command(async)]
pub fn camera_probe_auth(
    host: String,
    port: u16,
    username: String,
    password: String,
    brand: Option<String>,
    https: bool,
    timeout_ms: u64,
) -> Result<Value, String> {
    let scheme = if https { "https" } else { "http" };
    let base = format!("{scheme}://{host}:{port}");
    let out = Command::new("curl.exe")
        .args([
            "-sS",
            "-k",
            "--anyauth",
            "-u",
            &format!("{username}:{password}"),
            "--max-time",
            &(timeout_ms / 1000).max(1).to_string(),
            "-o",
            "NUL",
            "-w",
            "%{http_code}",
            &base,
        ])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    let code = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<u16>()
        .unwrap_or(0);
    let ok = (200..400).contains(&code);
    Ok(
        json!({"ok":ok,"message":if ok{"设备认证成功"}else{"认证失败或设备未提供 HTTP 管理接口"},"protocol":scheme.to_uppercase(),"vendor":brand.unwrap_or_else(||"auto".into()),"base_url":base,"status_code":code}),
    )
}
#[tauri::command(async)]
pub fn camera_get_device_info(
    host: String,
    port: u16,
    username: String,
    password: String,
    brand: Option<String>,
    https: bool,
    timeout_ms: u64,
) -> Result<Value, String> {
    let auth = camera_probe_auth(
        host.clone(),
        port,
        username.clone(),
        password.clone(),
        brand.clone(),
        https,
        timeout_ms,
    )?;
    if !auth["ok"].as_bool().unwrap_or(false) {
        return Ok(auth);
    }
    let scheme = if https { "https" } else { "http" };
    let vendor = brand.unwrap_or_else(|| "auto".into());
    let lower = vendor.to_ascii_lowercase();
    let endpoint = if lower.contains("dahua") || lower.contains("大华") {
        "/cgi-bin/magicBox.cgi?action=getSystemInfo"
    } else if lower.contains("uniview") || lower.contains("宇视") {
        "/LAPI/V1.0/System/DeviceInfo"
    } else {
        "/ISAPI/System/deviceInfo"
    };
    let url = format!("{scheme}://{host}:{port}{endpoint}");
    let raw = http_body(&url, &username, &password, timeout_ms).unwrap_or_default();
    let field = |names: &[&str]| -> Option<String> {
        for name in names {
            let open = format!("<{name}>");
            let close = format!("</{name}>");
            if let Some(start) = raw.find(&open) {
                let value_start = start + open.len();
                if let Some(end) = raw[value_start..].find(&close) {
                    return Some(raw[value_start..value_start + end].trim().to_string());
                }
            }
            for line in raw.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim().eq_ignore_ascii_case(name) {
                        return Some(value.trim().to_string());
                    }
                }
            }
        }
        None
    };
    Ok(
        json!({"ok":true,"message":"设备信息读取完成","device_name":field(&["deviceName","DeviceName"]).unwrap_or_else(||format!("网络摄像头 {host}")),"model":field(&["model","deviceModel","DeviceType"]),"serial":field(&["serialNumber","serialNo","SerialNumber"]),"firmware":field(&["firmwareVersion","softwareVersion","Version"]),"mac":field(&["macAddress","MAC"]),"protocol":auth["protocol"],"vendor":vendor,"raw":raw}),
    )
}

fn http_body(url: &str, user: &str, password: &str, timeout_ms: u64) -> Result<String, String> {
    let output = Command::new("curl.exe")
        .args([
            "-sS",
            "-k",
            "--anyauth",
            "-u",
            &format!("{user}:{password}"),
            "--max-time",
            &(timeout_ms / 1000).max(1).to_string(),
            url,
        ])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn http_call(
    method: &str,
    url: &str,
    user: &str,
    password: &str,
    body: Option<&str>,
    timeout_ms: u64,
) -> Result<u16, String> {
    let timeout = (timeout_ms / 1000).max(1).to_string();
    let mut command = Command::new("curl.exe");
    command.args([
        "-sS",
        "-k",
        "--anyauth",
        "-u",
        &format!("{user}:{password}"),
        "--max-time",
        &timeout,
        "-X",
        method,
        "-o",
        "NUL",
        "-w",
        "%{http_code}",
    ]);
    if let Some(content) = body {
        let content_type = if content.trim_start().starts_with('{') {
            "Content-Type: application/json"
        } else {
            "Content-Type: application/xml"
        };
        command.args(["-H", content_type, "--data-binary", "@-"]);
    }
    command
        .arg(url)
        .stdin(if body.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    if let Some(content) = body {
        child
            .stdin
            .take()
            .ok_or("无法写入设备请求")?
            .write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0))
}
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[tauri::command(async)]
pub fn camera_change_password(
    host: String,
    port: u16,
    username: String,
    old_password: String,
    new_password: String,
    brand: Option<String>,
    https: bool,
    timeout_ms: u64,
) -> Result<Value, String> {
    if new_password.len() < 8 {
        return Err("新密码至少 8 位（多数厂商要求）".into());
    }
    let scheme = if https { "https" } else { "http" };
    let b = brand.unwrap_or_else(|| "auto".into()).to_ascii_lowercase();
    let mut requests = Vec::<(String, &str, Option<String>)>::new();
    if b.contains("hik") || b.contains("海康") || b == "auto" {
        requests.push((
            format!("{scheme}://{host}:{port}/ISAPI/Security/users/1"),
            "PUT",
            Some(format!(
                "<User><id>1</id><userName>{}</userName><password>{}</password></User>",
                xml(&username),
                xml(&new_password)
            )),
        ));
    }
    if b.contains("dahua") || b.contains("大华") || b == "auto" {
        requests.push((format!("{scheme}://{host}:{port}/cgi-bin/userManager.cgi?action=modifyPassword&name={}&pwd={}&pwdOld={}",username,new_password,old_password),"GET",None));
    }
    if b.contains("uniview") || b.contains("宇视") || b == "auto" {
        requests.push((
            format!("{scheme}://{host}:{port}/LAPI/V1.0/System/Users/1/Password"),
            "PUT",
            Some(format!(
                r#"{{"OldPassword":"{}","NewPassword":"{}"}}"#,
                old_password, new_password
            )),
        ));
        requests.push((
            format!("{scheme}://{host}:{port}/LAPI/V1.0/System/Users/Password"),
            "PUT",
            Some(format!(
                r#"{{"UserName":"{}","OldPassword":"{}","NewPassword":"{}"}}"#,
                username, old_password, new_password
            )),
        ));
    }
    if b.contains("xm") || b.contains("雄迈") || b.contains("中维") || b == "auto" {
        requests.push((format!("{scheme}://{host}:{port}/cgi-bin/Account.cgi?action=modifyAccount&Account.Name={}&Account.Password={}",username,new_password),"GET",None));
        requests.push((
            format!(
                "{scheme}://{host}:{port}/cgi-bin/user/modify?user={}&oldpwd={}&newpwd={}",
                username, old_password, new_password
            ),
            "GET",
            None,
        ));
    }
    let mut code = 0_u16;
    let mut last_error = None;
    for (url, method, body) in requests {
        match http_call(
            method,
            &url,
            &username,
            &old_password,
            body.as_deref(),
            timeout_ms,
        ) {
            Ok(status) => {
                code = status;
                if (200..300).contains(&status) {
                    break;
                }
            }
            Err(error) => last_error = Some(error),
        }
    }
    let ok = (200..300).contains(&code);
    Ok(
        json!({"ok":ok,"message":if ok{"设备密码修改请求已成功执行"}else{"设备拒绝改密请求，请核对厂商、账号权限和密码策略"},"status_code":code,"vendor":b,"error":last_error}),
    )
}

fn upgrade_one(target: &Value, firmware: &str) -> Result<u16, String> {
    let host = target["host"].as_str().ok_or("升级目标缺少 host")?;
    let port = target["port"].as_u64().unwrap_or(80);
    let user = target["username"].as_str().unwrap_or("");
    let pass = target["password"].as_str().unwrap_or("");
    let https = target["https"].as_bool().unwrap_or(false);
    let scheme = if https { "https" } else { "http" };
    let brand = target["brand"]
        .as_str()
        .unwrap_or("auto")
        .to_ascii_lowercase();
    if brand.contains("uniview") || brand.contains("宇视") {
        let base = format!("{scheme}://{host}:{port}");
        let create = Command::new("curl.exe")
            .args([
                "-sS",
                "-k",
                "--anyauth",
                "-u",
                &format!("{user}:{pass}"),
                "--max-time",
                "60",
                "-H",
                "Content-Type: application/json",
                "-X",
                "POST",
                "--data-binary",
                r#"{"UpgradeType":3}"#,
                &format!("{base}/LAPI/V1.0/System/Upgrade"),
            ])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| e.to_string())?;
        if !create.status.success() {
            return Err(String::from_utf8_lossy(&create.stderr).trim().to_string());
        }
        let response: Value = serde_json::from_slice(&create.stdout).unwrap_or(Value::Null);
        let task = response
            .pointer("/UpgradeTaskID")
            .or_else(|| response.pointer("/Response/UpgradeTaskID"))
            .or_else(|| response.pointer("/Data/UpgradeTaskID"))
            .and_then(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.as_u64().map(|n| n.to_string()))
            })
            .ok_or("宇视设备未返回 UpgradeTaskID")?;
        let upload_url = format!("{base}/LAPI/V1.0/System/UploadFirmware?UpgradeTaskID={task}");
        let out = Command::new("curl.exe")
            .args([
                "-sS",
                "-k",
                "--anyauth",
                "-u",
                &format!("{user}:{pass}"),
                "--max-time",
                "300",
                "-X",
                "POST",
                "-F",
                &format!("file=@{firmware}"),
                "-o",
                "NUL",
                "-w",
                "%{http_code}",
                &upload_url,
            ])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        return Ok(String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .unwrap_or(0));
    }
    let endpoint = if brand.contains("hik") || brand.contains("海康") {
        "/ISAPI/System/updateFirmware"
    } else if brand.contains("dahua") || brand.contains("大华") {
        "/cgi-bin/magicBox.cgi?action=upgrade"
    } else {
        return Err(format!("未实现 {brand} 的升级传输协议"));
    };
    let out = Command::new("curl.exe")
        .args([
            "-sS",
            "-k",
            "--anyauth",
            "-u",
            &format!("{user}:{pass}"),
            "--max-time",
            "300",
            "-X",
            "POST",
            "-F",
            &format!("file=@{firmware}"),
            "-o",
            "NUL",
            "-w",
            "%{http_code}",
            &format!("{scheme}://{host}:{port}{endpoint}"),
        ])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0))
}

#[tauri::command(async)]
pub fn camera_start_batch_upgrade(
    app: AppHandle,
    state: State<'_, CameraState>,
    targets: Vec<Value>,
    firmware_path: String,
) -> Result<Value, String> {
    if !std::path::Path::new(&firmware_path).is_file() {
        return Err("固件文件不存在".into());
    }
    state.upgrade_cancel.store(false, Ordering::Relaxed);
    let started = Instant::now();
    let mut rows = Vec::new();
    for (index, target) in targets.iter().enumerate() {
        if state.upgrade_cancel.load(Ordering::Relaxed) {
            break;
        }
        let host = target["host"].as_str().unwrap_or("未知");
        let brand = target["brand"].as_str().unwrap_or("auto");
        let confirmed = target["confirmed"].as_bool().unwrap_or(false);
        let result = if confirmed {
            upgrade_one(target, &firmware_path)
        } else {
            Err("未确认型号/硬件版本匹配".into())
        };
        let row = match result {
            Ok(code) if (200..300).contains(&code) => {
                json!({"index":index,"host":host,"vendor":brand,"model":"已确认","firmware_before":"未知","firmware_after":"待设备重启后核对","phase":"done","message":format!("固件上传完成，HTTP {code}")})
            }
            Ok(code) => {
                json!({"index":index,"host":host,"vendor":brand,"model":"未知","firmware_before":"未知","firmware_after":"未知","phase":"failed","message":format!("设备拒绝升级，HTTP {code}")})
            }
            Err(e) => {
                json!({"index":index,"host":host,"vendor":brand,"model":"未知","firmware_before":"未知","firmware_after":"未知","phase":"failed","message":e})
            }
        };
        let _ = app.emit("camera:upgrade-row", &row);
        let _ = app.emit(
            "camera:upgrade-line",
            format!("{} · {}", host, row["message"].as_str().unwrap_or("")),
        );
        rows.push(row);
        if rows.last().is_some_and(|r| r["phase"] == "failed") {
            break;
        }
    }
    let success = rows.iter().filter(|r| r["phase"] == "done").count();
    let failed = rows.iter().filter(|r| r["phase"] == "failed").count();
    let result = json!({"total":targets.len(),"success":success,"failed":failed,"skipped":targets.len().saturating_sub(rows.len()),"elapsed_ms":started.elapsed().as_millis(),"rows":rows});
    let _ = app.emit("camera:upgrade-complete", &result);
    Ok(result)
}
#[tauri::command(async)]
pub fn camera_stop_batch_upgrade(state: State<'_, CameraState>) -> Value {
    state.upgrade_cancel.store(true, Ordering::Relaxed);
    json!({"success":true,"message":"已请求在当前设备操作结束后停止批量升级"})
}
