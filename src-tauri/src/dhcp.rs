use serde_json::json;
use std::net::UdpSocket;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

fn ip(bytes: &[u8]) -> String {
    format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
}
fn options(data: &[u8]) -> std::collections::HashMap<u8, Vec<u8>> {
    let mut result = std::collections::HashMap::new();
    let mut i = 240;
    while i < data.len() {
        let code = data[i];
        i += 1;
        if code == 255 {
            break;
        }
        if code == 0 {
            continue;
        }
        if i >= data.len() {
            break;
        }
        let len = data[i] as usize;
        i += 1;
        if i + len > data.len() {
            break;
        }
        result.insert(code, data[i..i + len].to_vec());
        i += len;
    }
    result
}

#[tauri::command]
pub fn start_dhcp_detect(
    app: AppHandle,
    iface_ip: String,
    mac: String,
    timeout_secs: u64,
) -> Result<bool, String> {
    let mac_bytes = mac
        .split(|c| c == ':' || c == '-')
        .map(|s| u8::from_str_radix(s, 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "网卡 MAC 格式无效")?;
    if mac_bytes.len() != 6 {
        return Err("网卡 MAC 格式无效".into());
    }
    let socket = UdpSocket::bind("0.0.0.0:68")
        .map_err(|e| format!("绑定 DHCP 客户端端口 UDP 68 失败（请以管理员运行）：{e}"))?;
    socket.set_broadcast(true).map_err(|e| e.to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_millis(300)))
        .map_err(|e| e.to_string())?;
    let xid = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u32)
        .to_be_bytes();
    let mut packet = vec![0u8; 240];
    packet[0] = 1;
    packet[1] = 1;
    packet[2] = 6;
    packet[4..8].copy_from_slice(&xid);
    packet[10] = 0x80;
    packet[28..34].copy_from_slice(&mac_bytes);
    packet[236..240].copy_from_slice(&[99, 130, 83, 99]);
    packet.extend_from_slice(&[53, 1, 1, 55, 8, 1, 3, 6, 15, 51, 54, 58, 59, 255]);
    socket
        .send_to(&packet, "255.255.255.255:67")
        .map_err(|e| format!("发送 DHCP Discover 失败：{e}"))?;
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut offers = Vec::new();
        let mut buf = [0u8; 2048];
        let _=app.emit("dhcp:progress",json!({"progress":10,"message":format!("已从 {iface_ip} 广播 DHCP Discover，等待 Offer…")}));
        while started.elapsed() < Duration::from_secs(timeout_secs.clamp(1, 60)) {
            if let Ok((n, from)) = socket.recv_from(&mut buf) {
                if n >= 240 && buf[0] == 2 && buf[4..8] == xid {
                    let opts = options(&buf[..n]);
                    if opts.get(&53).and_then(|v| v.first()) == Some(&2) {
                        let offered = ip(&buf[16..20]);
                        let server = opts
                            .get(&54)
                            .filter(|v| v.len() >= 4)
                            .map(|v| ip(v))
                            .unwrap_or_else(|| from.ip().to_string());
                        let subnet = opts.get(&1).filter(|v| v.len() >= 4).map(|v| ip(v));
                        let router = opts.get(&3).filter(|v| v.len() >= 4).map(|v| ip(v));
                        let dns = opts
                            .get(&6)
                            .map(|v| v.chunks_exact(4).map(ip).collect::<Vec<_>>())
                            .unwrap_or_default();
                        let lease = opts
                            .get(&51)
                            .filter(|v| v.len() >= 4)
                            .map(|v| u32::from_be_bytes(v[..4].try_into().unwrap()))
                            .unwrap_or(0);
                        offers.push(json!({"server_id":server,"offered_ip":offered,"subnet_mask":subnet,"router":router,"dns":dns,"lease_secs":lease,"source":from.to_string()}));
                    }
                }
            }
            let progress = (started.elapsed().as_secs_f64() / timeout_secs.max(1) as f64 * 90.0
                + 10.0)
                .min(99.0);
            let _=app.emit("dhcp:progress",json!({"progress":progress,"message":format!("等待 DHCP Offer，已发现 {} 台服务器",offers.len())}));
        }
        let result = json!({"ok":true,"offers":offers,"elapsed_ms":started.elapsed().as_millis(),"error":null});
        let _ = app.emit("dhcp:complete", result);
    });
    Ok(true)
}
