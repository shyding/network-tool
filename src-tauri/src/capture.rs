use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct CaptureState {
    inner: Mutex<CaptureData>,
}
#[derive(Default)]
struct CaptureData {
    running: bool,
    etl: Option<PathBuf>,
    pcap: Option<PathBuf>,
    packets: Vec<Value>,
    max: usize,
}
fn run(args: &[&str]) -> Result<String, String> {
    let out = Command::new("pktmon.exe")
        .args(args)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
fn run_owned(args: &[String]) -> Result<String, String> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run(&refs)
}

pub(crate) fn component_id(device: &str) -> Option<String> {
    let wanted = device.trim().to_ascii_lowercase();
    if wanted.is_empty() {
        return None;
    }
    run(&["list"]).ok()?.lines().find_map(|line| {
        let lower = line.to_ascii_lowercase();
        if !lower.contains(&wanted) {
            return None;
        }
        line.split_whitespace()
            .find(|part| part.chars().all(|c| c.is_ascii_digit()))
            .map(str::to_owned)
    })
}

fn configure_filter(filter: &str) -> Result<(), String> {
    run(&["filter", "remove"])?;
    let filter = filter.trim();
    if filter.is_empty() {
        return Ok(());
    }
    for (index, clause) in filter.split(" or ").enumerate() {
        let tokens = clause.split_whitespace().collect::<Vec<_>>();
        let mut args = vec![
            "filter".to_string(),
            "add".to_string(),
            format!("toolbox{}", index + 1),
        ];
        let mut i = 0;
        while i < tokens.len() {
            match tokens[i].to_ascii_lowercase().as_str() {
                "and" | "src" | "dst" => i += 1,
                "tcp" | "udp" | "icmp" | "icmpv6" => {
                    args.extend(["-t".to_string(), tokens[i].to_ascii_uppercase()]);
                    i += 1;
                }
                "host" | "net" => {
                    let value = tokens.get(i + 1).ok_or("BPF host/net 后缺少地址")?;
                    if !value
                        .chars()
                        .all(|c| c.is_ascii_hexdigit() || ".:/".contains(c))
                    {
                        return Err(format!("BPF 地址无效：{value}"));
                    }
                    args.extend(["-i".to_string(), (*value).to_string()]);
                    i += 2;
                }
                "port" => {
                    let value = tokens.get(i + 1).ok_or("BPF port 后缺少端口")?;
                    let port = value
                        .parse::<u16>()
                        .map_err(|_| format!("BPF 端口无效：{value}"))?;
                    args.extend(["-p".to_string(), port.to_string()]);
                    i += 2;
                }
                other => return Err(format!("当前抓包引擎不支持该 BPF 片段：{other}")),
            }
        }
        if args.len() == 3 {
            return Err("BPF 过滤条件为空".into());
        }
        run_owned(&args).map_err(|e| format!("设置 BPF 过滤器失败：{e}"))?;
    }
    Ok(())
}
#[tauri::command(async)]
pub fn capture_status(state: State<'_, CaptureState>) -> Value {
    let running = state.inner.lock().map(|s| s.running).unwrap_or(false);
    let available = Command::new("where.exe")
        .arg("pktmon.exe")
        .creation_flags(0x08000000)
        .output()
        .is_ok_and(|o| o.status.success());
    json!({"available":available,"running":running,"message":if available{"Windows PktMon 抓包引擎已就绪（兼容 Npcap 页面）"}else{"系统未提供 PktMon"}})
}
#[tauri::command]
pub async fn list_capture_ifaces() -> Result<Value, String> {
    tokio::task::spawn_blocking(|| Ok(json!(crate::interfaces::all_interfaces()?.into_iter().map(|i|json!({"key":i["id"],"label":format!("{} · {}",i["name"].as_str().unwrap_or("网卡"),i["ipv4"].as_str().unwrap_or("")),"device":i["description"].as_str().unwrap_or_else(||i["name"].as_str().unwrap_or("")),"name":i["name"]})).collect::<Vec<_>>())))
        .await
        .map_err(|error| format!("读取抓包网卡任务失败：{error}"))?
}
#[tauri::command(async)]
pub fn start_packet_capture(
    state: State<'_, CaptureState>,
    device: String,
    filter: String,
    max_packets: usize,
) -> Result<bool, String> {
    let base = std::env::temp_dir().join(format!("network-toolbox-capture-{}", std::process::id()));
    let etl = base.with_extension("etl");
    let pcap = base.with_extension("pcapng");
    let _ = std::fs::remove_file(&etl);
    let _ = std::fs::remove_file(&pcap);
    configure_filter(&filter)?;
    let mut args = vec![
        "start".to_string(),
        "--capture".to_string(),
        "--pkt-size".to_string(),
        "0".to_string(),
        "--file-name".to_string(),
        etl.to_string_lossy().into_owned(),
    ];
    if let Some(id) = component_id(&device) {
        args.extend(["--comp".to_string(), id]);
    } else {
        // Still limit capture to NIC components. This is preferable to silently
        // including every internal Windows networking component.
        args.extend(["--comp".to_string(), "nics".to_string()]);
    }
    run_owned(&args).map_err(|e| format!("启动抓包失败（通常需要管理员权限）：{e}"))?;
    let mut d = state.inner.lock().map_err(|_| "抓包状态锁异常")?;
    d.running = true;
    d.etl = Some(etl);
    d.pcap = Some(pcap);
    d.packets.clear();
    d.max = max_packets.clamp(10, 50_000);
    Ok(true)
}
#[tauri::command(async)]
pub fn stop_packet_capture(app: AppHandle, state: State<'_, CaptureState>) -> Result<bool, String> {
    run(&["stop"])?;
    let (etl, pcap, max) = {
        let d = state.inner.lock().map_err(|_| "抓包状态锁异常")?;
        (
            d.etl.clone().ok_or("没有活动抓包")?,
            d.pcap.clone().ok_or("没有输出路径")?,
            d.max,
        )
    };
    run(&[
        "etl2pcap",
        etl.to_string_lossy().as_ref(),
        "--out",
        pcap.to_string_lossy().as_ref(),
    ])?;
    let packets = parse_capture(&pcap, max)?;
    let stats = stats(&packets);
    for packet in &packets {
        let _ = app.emit("capture:packet", packet);
    }
    let _ = app.emit("capture:stats", &stats);
    let _ = app.emit("capture:complete", json!({"ok":true,"total":packets.len()}));
    let mut d = state.inner.lock().map_err(|_| "抓包状态锁异常")?;
    d.running = false;
    d.packets = packets;
    Ok(true)
}
#[tauri::command(async)]
pub fn clear_packet_capture(state: State<'_, CaptureState>) -> Result<bool, String> {
    let mut d = state.inner.lock().map_err(|_| "抓包状态锁异常")?;
    if d.running {
        return Err("请先停止抓包".into());
    }
    d.packets.clear();
    if let Some(p) = d.pcap.take() {
        let _ = std::fs::remove_file(p);
    }
    if let Some(p) = d.etl.take() {
        let _ = std::fs::remove_file(p);
    }
    Ok(true)
}
#[tauri::command(async)]
pub fn export_packet_capture(
    state: State<'_, CaptureState>,
    path: String,
) -> Result<usize, String> {
    let d = state.inner.lock().map_err(|_| "抓包状态锁异常")?;
    let source = d.pcap.as_ref().ok_or("没有可导出的抓包文件")?;
    let target = PathBuf::from(path);
    if target
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pcap"))
    {
        write_classic_pcap(&target, &read_frames(source, usize::MAX)?)?;
    } else {
        std::fs::copy(source, &target).map_err(|e| format!("导出失败：{e}"))?;
    }
    Ok(d.packets.len())
}
#[tauri::command(async)]
pub fn import_packet_capture(
    state: State<'_, CaptureState>,
    path: String,
) -> Result<Value, String> {
    let source = PathBuf::from(path);
    let packets = parse_capture(&source, 50_000)?;
    let result = json!({"packets":packets,"stats":stats(&packets),"message":format!("已导入 {} 个数据包",packets.len()),"truncated":packets.len()>=50_000});
    let mut d = state.inner.lock().map_err(|_| "抓包状态锁异常")?;
    d.pcap = Some(source);
    d.packets = packets;
    Ok(result)
}

fn stats(packets: &[Value]) -> Value {
    let mut tcp = 0;
    let mut udp = 0;
    let mut icmp = 0;
    for p in packets {
        match p["transport"].as_str() {
            Some("TCP") => tcp += 1,
            Some("UDP") => udp += 1,
            Some("ICMP") | Some("ICMPv6") => icmp += 1,
            _ => {}
        }
    }
    json!({"total":packets.len(),"tcp":tcp,"udp":udp,"icmp":icmp,"other":packets.len().saturating_sub(tcp+udp+icmp)})
}

pub fn ai_context(state: &CaptureState) -> Result<String, String> {
    let data = state.inner.lock().map_err(|_| "抓包状态锁异常")?;
    let summary = stats(&data.packets);
    let examples = data.packets.iter().take(100).cloned().collect::<Vec<_>>();
    Ok(json!({"stats":summary,"first_packets":examples}).to_string())
}
pub fn engine_running(state: &CaptureState) -> bool {
    state.inner.lock().map(|d| d.running).unwrap_or(false)
}
fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn u32be(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn read_frames(path: &PathBuf, max: usize) -> Result<Vec<Vec<u8>>, String> {
    let data = std::fs::read(path).map_err(|e| format!("读取抓包文件失败：{e}"))?;
    if data.len() < 12 {
        return Err("抓包文件过短".into());
    }
    let mut frames = Vec::<Vec<u8>>::new();
    if u32le(&data, 0) == Some(0x0A0D0D0A) {
        let mut o = 0;
        while o + 28 <= data.len() && frames.len() < max {
            let typ = u32le(&data, o).unwrap_or(0);
            let len = u32le(&data, o + 4).unwrap_or(0) as usize;
            if len < 12 || o + len > data.len() {
                break;
            }
            if typ == 6 {
                let cap = u32le(&data, o + 20).unwrap_or(0) as usize;
                if o + 28 + cap <= data.len() {
                    frames.push(data[o + 28..o + 28 + cap].to_vec())
                }
            }
            o += len;
        }
    } else {
        let magic = data.get(0..4).ok_or("PCAP 文件头损坏")?;
        let little = matches!(magic, [0xd4, 0xc3, 0xb2, 0xa1] | [0x4d, 0x3c, 0xb2, 0xa1]);
        let big = matches!(magic, [0xa1, 0xb2, 0xc3, 0xd4] | [0xa1, 0xb2, 0x3c, 0x4d]);
        if !little && !big {
            return Err("不支持的 PCAP 文件格式".into());
        }
        let mut o = 24;
        while o + 16 <= data.len() && frames.len() < max {
            let cap = if little {
                u32le(&data, o + 8)
            } else {
                u32be(&data, o + 8)
            }
            .unwrap_or(0) as usize;
            if o + 16 + cap > data.len() {
                break;
            }
            frames.push(data[o + 16..o + 16 + cap].to_vec());
            o += 16 + cap;
        }
    }
    Ok(frames)
}
fn parse_capture(path: &PathBuf, max: usize) -> Result<Vec<Value>, String> {
    Ok(read_frames(path, max)?
        .iter()
        .enumerate()
        .map(|(i, f)| decode(i + 1, f))
        .collect())
}
fn write_classic_pcap(path: &PathBuf, frames: &[Vec<u8>]) -> Result<(), String> {
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut file = std::fs::File::create(path).map_err(|e| format!("创建 PCAP 失败：{e}"))?;
    file.write_all(&[
        0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0, 0, 1, 0, 0, 0,
    ])
    .map_err(|e| e.to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    for (index, frame) in frames.iter().enumerate() {
        let secs = now.as_secs().saturating_add((index / 1_000_000) as u64) as u32;
        let usecs = (now.subsec_micros() as usize + index) % 1_000_000;
        let len = u32::try_from(frame.len()).map_err(|_| "数据包过大")?;
        file.write_all(&secs.to_le_bytes())
            .map_err(|e| e.to_string())?;
        file.write_all(&(usecs as u32).to_le_bytes())
            .map_err(|e| e.to_string())?;
        file.write_all(&len.to_le_bytes())
            .map_err(|e| e.to_string())?;
        file.write_all(&len.to_le_bytes())
            .map_err(|e| e.to_string())?;
        file.write_all(frame).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn decode(no: usize, f: &[u8]) -> Value {
    let mut src = String::new();
    let mut dst = String::new();
    let mut transport = "OTHER";
    let mut info = String::new();
    if f.len() >= 14 {
        let eth = u16::from_be_bytes([f[12], f[13]]);
        if eth == 0x0800 && f.len() >= 34 {
            src = format!("{}.{}.{}.{}", f[26], f[27], f[28], f[29]);
            dst = format!("{}.{}.{}.{}", f[30], f[31], f[32], f[33]);
            let ihl = ((f[14] & 15) as usize) * 4;
            let p = f[23];
            transport = match p {
                6 => "TCP",
                17 => "UDP",
                1 => "ICMP",
                _ => "IPv4",
            };
            if (p == 6 || p == 17) && f.len() >= 14 + ihl + 4 {
                let a = 14 + ihl;
                info = format!(
                    "{} → {}",
                    u16::from_be_bytes([f[a], f[a + 1]]),
                    u16::from_be_bytes([f[a + 2], f[a + 3]])
                );
            }
        } else if eth == 0x86dd && f.len() >= 54 {
            transport = match f[20] {
                6 => "TCP",
                17 => "UDP",
                58 => "ICMPv6",
                _ => "IPv6",
            };
            src = "IPv6".into();
            dst = "IPv6".into();
        } else {
            transport = "Ethernet";
        }
    }
    json!({"no":no,"time":format!("#{no}"),"src":src,"dst":dst,"transport":transport,"length":f.len(),"info":info})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tcp_frame() -> Vec<u8> {
        let mut frame = vec![0_u8; 54];
        frame[12..14].copy_from_slice(&0x0800_u16.to_be_bytes());
        frame[14] = 0x45;
        frame[23] = 6;
        frame[26..30].copy_from_slice(&[192, 168, 1, 10]);
        frame[30..34].copy_from_slice(&[1, 1, 1, 1]);
        frame[34..36].copy_from_slice(&50_000_u16.to_be_bytes());
        frame[36..38].copy_from_slice(&443_u16.to_be_bytes());
        frame
    }

    #[test]
    fn decodes_ethernet_ipv4_tcp() {
        let packet = decode(1, &tcp_frame());
        assert_eq!(packet["src"], "192.168.1.10");
        assert_eq!(packet["dst"], "1.1.1.1");
        assert_eq!(packet["transport"], "TCP");
        assert_eq!(packet["info"], "50000 → 443");
    }

    #[test]
    fn classic_pcap_export_round_trips() {
        let path = std::env::temp_dir().join(format!(
            "network-toolbox-test-{}-{}.pcap",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let frames = vec![tcp_frame()];
        write_classic_pcap(&path, &frames).unwrap();
        let parsed = read_frames(&path, 10).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(parsed, frames);
    }
}
