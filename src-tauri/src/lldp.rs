use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
#[derive(Default)]
pub struct LldpState {
    running: Arc<AtomicBool>,
    neighbors: Arc<Mutex<Vec<Value>>>,
}
fn run(args: &[&str]) -> Result<(), String> {
    let out = Command::new("pktmon.exe")
        .args(args)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
fn run_owned(args: &[String]) -> Result<(), String> {
    run(&args.iter().map(String::as_str).collect::<Vec<_>>())
}
#[tauri::command]
pub fn lldp_status(state: State<'_, LldpState>) -> Value {
    let available = Command::new("where.exe")
        .arg("pktmon.exe")
        .creation_flags(0x08000000)
        .output()
        .is_ok_and(|o| o.status.success());
    json!({"available":available,"running":state.running.load(Ordering::Relaxed),"message":if available{"Windows PktMon 二层捕获引擎已就绪"}else{"未找到二层捕获引擎"}})
}
#[tauri::command]
pub fn list_lldp_neighbors(state: State<'_, LldpState>) -> Result<Value, String> {
    Ok(Value::Array(
        state
            .neighbors
            .lock()
            .map_err(|_| "LLDP 状态锁异常")?
            .clone(),
    ))
}
#[tauri::command]
pub fn stop_lldp_scan(state: State<'_, LldpState>) -> bool {
    state.running.store(false, Ordering::Relaxed);
    true
}
#[tauri::command]
pub fn start_lldp_scan(
    app: AppHandle,
    state: State<'_, LldpState>,
    capture: State<'_, crate::capture::CaptureState>,
    device: String,
    duration_secs: u64,
) -> Result<bool, String> {
    if !crate::firewall::elevated() {
        return Err("终端端口扫描需要管理员权限，请点击“提权重启”后再试".into());
    }
    if crate::capture::engine_running(&capture) {
        return Err("数据包抓包正在运行，请先停止后再监听 LLDP".into());
    }
    let base = std::env::temp_dir().join(format!("network-toolbox-lldp-{}", std::process::id()));
    let etl = base.with_extension("etl");
    let pcap = base.with_extension("pcapng");
    let _ = std::fs::remove_file(&etl);
    let _ = std::fs::remove_file(&pcap);
    let _ = run(&["filter", "remove"]);
    run(&["filter", "add", "NetworkToolboxLLDP", "-d", "35020"])?;
    run(&[
        "filter",
        "add",
        "NetworkToolboxCDP",
        "-m",
        "01:00:0C:CC:CC:CC",
    ])?;
    let args = vec!["start".into(),"--capture".into(),"--comp".into(),crate::capture::component_id(&device).unwrap_or_else(||"nics".into()),"--pkt-size".into(),"0".into(),"--file-name".into(),etl.to_string_lossy().into_owned()];
    run_owned(&args)
    .map_err(|e| format!("启动 LLDP 监听失败（需要管理员权限）：{e}"))?;
    state.running.store(false, Ordering::Relaxed);
    state
        .neighbors
        .lock()
        .map_err(|_| "LLDP 状态锁异常")?
        .clear();
    let running = state.running.clone();
    let neighbors = state.neighbors.clone();
    running.store(true, Ordering::Relaxed);
    std::thread::spawn(move || {
        let started = Instant::now();
        while running.load(Ordering::Relaxed)
            && started.elapsed() < Duration::from_secs(duration_secs.clamp(1, 300))
        {
            let _=app.emit("lldp:progress",json!({"message":format!("正在监听 {}，剩余 {:.0} 秒",device,(duration_secs as f64-started.elapsed().as_secs_f64()).max(0.0))}));
            std::thread::sleep(Duration::from_millis(500));
        }
        let _ = run(&["stop"]);
        let _ = run(&["filter", "remove"]);
        let convert = run(&[
            "etl2pcap",
            etl.to_string_lossy().as_ref(),
            "--out",
            pcap.to_string_lossy().as_ref(),
        ]);
        let parsed = convert.and_then(|_| parse_file(&pcap)).unwrap_or_default();
        for neighbor in &parsed {
            let _ = app.emit("lldp:neighbor", neighbor);
        }
        if let Ok(mut n) = neighbors.lock() {
            *n = parsed.clone();
        }
        running.store(false, Ordering::Relaxed);
        let _ = app.emit("lldp:complete", json!({"ok":true,"count":parsed.len()}));
    });
    Ok(true)
}
fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn parse_file(path: &PathBuf) -> Result<Vec<Value>, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    let mut o = 0;
    while o + 28 <= data.len() {
        let typ = u32le(&data, o).unwrap_or(0);
        let len = u32le(&data, o + 4).unwrap_or(0) as usize;
        if len < 12 || o + len > data.len() {
            break;
        }
        if typ == 6 {
            let cap = u32le(&data, o + 20).unwrap_or(0) as usize;
            if o + 28 + cap <= data.len() {
                if let Some(v) = parse_frame(&data[o + 28..o + 28 + cap]) {
                    let uid = v["uid"].clone();
                    if !result.iter().any(|x: &Value| x["uid"] == uid) {
                        result.push(v);
                    }
                }
            }
        }
        o += len;
    }
    Ok(result)
}
fn id_value(subtype: u8, data: &[u8]) -> String {
    if subtype == 4 && data.len() == 6 {
        data.iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    } else {
        String::from_utf8_lossy(data)
            .trim_matches(char::from(0))
            .to_string()
    }
}
fn parse_frame(frame: &[u8]) -> Option<Value> {
    if frame.len() >= 26
        && frame[0..6] == [0x01, 0x00, 0x0c, 0xcc, 0xcc, 0xcc]
        && frame[14..22] == [0xaa, 0xaa, 0x03, 0x00, 0x00, 0x0c, 0x20, 0x00]
    {
        return parse_cdp(frame);
    }
    if frame.len() < 16 || u16::from_be_bytes([frame[12], frame[13]]) != 0x88cc {
        return None;
    }
    let mut chassis = String::new();
    let mut port = String::new();
    let mut ttl = 0u16;
    let mut port_desc = String::new();
    let mut system_name = String::new();
    let mut system_desc = String::new();
    let mut capabilities = String::new();
    let mut management = String::new();
    let mut vlan: Option<u16> = None;
    let mut o = 14;
    while o + 2 <= frame.len() {
        let h = u16::from_be_bytes([frame[o], frame[o + 1]]);
        o += 2;
        let typ = (h >> 9) as u8;
        let len = (h & 0x1ff) as usize;
        if typ == 0 || o + len > frame.len() {
            break;
        }
        let v = &frame[o..o + len];
        match typ {
            1 if len > 1 => chassis = id_value(v[0], &v[1..]),
            2 if len > 1 => port = id_value(v[0], &v[1..]),
            3 if len >= 2 => ttl = u16::from_be_bytes([v[0], v[1]]),
            4 => port_desc = String::from_utf8_lossy(v).into_owned(),
            5 => system_name = String::from_utf8_lossy(v).into_owned(),
            6 => system_desc = String::from_utf8_lossy(v).into_owned(),
            7 if len >= 4 => capabilities = format!("0x{:02X}{:02X}", v[2], v[3]),
            8 if len > 2 => {
                management = if v[1] == 1 && len >= 6 {
                    format!("{}.{}.{}.{}", v[2], v[3], v[4], v[5])
                } else {
                    "其他".into()
                }
            }
            127 if len >= 6 && v[0..3] == [0x00, 0x80, 0xc2] && v[3] == 1 => {
                vlan = Some(u16::from_be_bytes([v[4], v[5]]));
            }
            _ => {}
        }
        o += len;
    }
    if chassis.is_empty() && port.is_empty() {
        None
    } else {
        let switch_name = if system_name.is_empty(){chassis.clone()}else{system_name.clone()};
        let last_seen = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs().to_string();
        Some(json!({"uid":format!("{chassis}|{port}"),"protocol":"LLDP","switch_name":switch_name,"chassis_id":chassis,"port_id":port,"ttl":ttl,"port_desc":port_desc,"port_description":port_desc,"system_name":system_name,"sys_desc":system_desc,"system_description":system_desc,"capabilities":capabilities,"mgmt_ip":management,"management_address":management,"vlan":vlan.map(|value|value.to_string()).unwrap_or_else(||"—".into()),"last_seen":last_seen}))
    }
}

fn parse_cdp(frame: &[u8]) -> Option<Value> {
    let ttl = *frame.get(23)? as u16;
    let mut chassis = String::new();
    let mut port = String::new();
    let mut sys_desc = String::new();
    let mut platform = String::new();
    let mut vlan: Option<u16> = None;
    let mut capabilities = String::new();
    let mut offset = 26;
    while offset + 4 <= frame.len() {
        let typ = u16::from_be_bytes([frame[offset], frame[offset + 1]]);
        let len = u16::from_be_bytes([frame[offset + 2], frame[offset + 3]]) as usize;
        if len < 4 || offset + len > frame.len() { break; }
        let value = &frame[offset + 4..offset + len];
        match typ {
            1 => chassis = String::from_utf8_lossy(value).trim().to_string(),
            3 => port = String::from_utf8_lossy(value).trim().to_string(),
            4 if value.len() >= 4 => capabilities = format!("0x{:08X}",u32::from_be_bytes(value[..4].try_into().ok()?)),
            5 => sys_desc = String::from_utf8_lossy(value).trim().to_string(),
            6 => platform = String::from_utf8_lossy(value).trim().to_string(),
            10 if value.len() >= 2 => vlan = Some(u16::from_be_bytes([value[0],value[1]])),
            _ => {}
        }
        offset += len;
    }
    if chassis.is_empty() && port.is_empty() { return None; }
    let description = if platform.is_empty(){sys_desc.clone()}else{format!("{platform} · {sys_desc}")};
    Some(json!({"uid":format!("CDP|{chassis}|{port}"),"protocol":"CDP","switch_name":chassis,"chassis_id":chassis,"port_id":port,"ttl":ttl,"port_desc":port,"port_description":port,"system_name":chassis,"sys_desc":description,"system_description":description,"capabilities":capabilities,"mgmt_ip":"","management_address":"","vlan":vlan.map(|value|value.to_string()).unwrap_or_else(||"—".into()),"last_seen":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs().to_string()}))
}
