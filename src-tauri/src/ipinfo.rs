use serde_json::{json, Value};
use std::net::IpAddr;
use std::os::windows::process::CommandExt as StdCommandExt;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;

const WIFI_TIMEOUT: Duration = Duration::from_secs(8);
static WIFI_SCAN: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

#[link(name = "kernel32")]
extern "system" {
    fn MultiByteToWideChar(code_page: u32, flags: u32, input: *const u8, input_len: i32, output: *mut u16, output_len: i32) -> i32;
}

fn decode_netsh_output(bytes: &[u8]) -> Result<String, String> {
    if let Ok(text) = std::str::from_utf8(bytes) { return Ok(text.trim_start_matches('\u{feff}').into()); }
    let length = i32::try_from(bytes.len()).map_err(|_| "netsh output is too large")?;
    // netsh uses the Windows OEM code page when stdout is redirected.
    let required = unsafe { MultiByteToWideChar(1, 0, bytes.as_ptr(), length, std::ptr::null_mut(), 0) };
    if required == 0 { return Err("Unable to decode netsh output".into()); }
    let mut wide = vec![0_u16; required as usize];
    let written = unsafe { MultiByteToWideChar(1, 0, bytes.as_ptr(), length, wide.as_mut_ptr(), required) };
    if written == 0 { return Err("Unable to decode netsh output".into()); }
    Ok(String::from_utf16_lossy(&wide[..written as usize]))
}

async fn wifi_command_output(mut command: Command, wait: Duration) -> Result<std::process::Output, String> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .creation_flags(0x08000000).kill_on_drop(true);
    tokio::time::timeout(wait, command.output()).await
        .map_err(|_| "WiFi 查询超时，已终止查询；请检查无线网卡或 WLAN 服务后重试".to_string())?
        .map_err(|error| format!("WiFi 查询启动失败：{error}"))
}

async fn wifi_lines(arguments: &[String]) -> Result<Vec<String>, String> {
    let mut command = Command::new("netsh.exe");
    command.args(arguments);
    let output = wifi_command_output(command, WIFI_TIMEOUT).await?;
    let text = decode_netsh_output(&output.stdout)?;
    if !output.status.success() {
        let error = decode_netsh_output(&output.stderr)?;
        return Err(format!("WiFi 查询失败：{}", if text.trim().is_empty() { error.trim() } else { text.trim() }));
    }
    Ok(text.lines().map(str::to_string).collect())
}

fn wifi_scan_arguments(interface: Option<&str>) -> Result<Vec<String>, String> {
    let mut arguments = vec!["wlan".into(), "show".into(), "networks".into(), "mode=bssid".into()];
    if let Some(name) = interface.map(str::trim).filter(|name| !name.is_empty()) {
        if name.len() > 256 || name.chars().any(char::is_control) { return Err("无线网卡名称无效".into()); }
        arguments.push(format!("interface={name}"));
    }
    Ok(arguments)
}

fn netsh_lines(arguments: &str) -> Result<Vec<String>, String> {
    let script = format!(
        r#"[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); @(& netsh.exe {arguments}) | ConvertTo-Json -Compress"#
    );
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ])
        .creation_flags(0x08000000)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("解析 netsh 输出失败：{error}"))?;
    Ok(match value {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        Value::String(s) => vec![s],
        _ => Vec::new(),
    })
}

fn after_colon(line: &str) -> &str {
    line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("")
}

fn private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

async fn remote_lookup(ip: Option<&str>) -> Result<Value, String> {
    let url = match ip {
        Some(value) => format!("https://ipwho.is/{value}?fields=success,message,ip,country,region,city,latitude,longitude,connection"),
        None => "https://ipwho.is/?fields=success,message,ip,country,region,city,latitude,longitude,connection".into(),
    };
    let started = Instant::now();
    let output = Command::new("curl.exe")
        .args(["-sS", "--max-time", "8", &url])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .output()
        .await
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("公网 IP 服务暂时不可用".into());
    }
    let raw: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "公网 IP 服务返回无效数据".to_string())?;
    if raw["success"].as_bool() == Some(false) {
        return Err(raw["message"].as_str().unwrap_or("IP 查询失败").into());
    }
    let country = raw["country"].as_str().unwrap_or("");
    let region = raw["region"].as_str().unwrap_or("");
    let city = raw["city"].as_str().unwrap_or("");
    let isp = raw["connection"]["isp"].as_str().unwrap_or("");
    Ok(json!({
        "ip": raw["ip"],
        "is_private": false,
        "elapsed_ms": started.elapsed().as_millis(),
        "location": {"label": format!("{country} {region} {city}").trim(), "country": country, "region": region, "city": city, "isp": isp,
            "latitude": raw["latitude"], "longitude": raw["longitude"]}
    }))
}

#[tauri::command]
pub async fn get_public_ip_info() -> Result<Value, String> {
    remote_lookup(None).await
}

#[tauri::command]
pub async fn query_ip_info(ip: String) -> Result<Value, String> {
    let parsed: IpAddr = ip.parse().map_err(|_| "IP 地址格式无效".to_string())?;
    if private_ip(parsed) {
        return Ok(
            json!({"ip": ip, "is_private": true, "elapsed_ms": 0, "location": {"label": "私有/内网地址"}}),
        );
    }
    remote_lookup(Some(&ip)).await
}

#[tauri::command]
pub fn convert_mac_format(mac: String) -> Result<Value, String> {
    let plain = mac
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>()
        .to_uppercase();
    if plain.len() != 12 {
        return Err("MAC 地址应包含 12 个十六进制字符".into());
    }
    let pairs = (0..6)
        .map(|index| &plain[index * 2..index * 2 + 2])
        .collect::<Vec<_>>();
    Ok(json!({
        "colon": pairs.join(":"), "hyphen": pairs.join("-"),
        "dot": format!("{}.{}.{}", &plain[0..4], &plain[4..8], &plain[8..12]), "plain": plain,
    }))
}

#[tauri::command]
pub async fn lookup_mac_vendor(mac: String) -> Result<Value, String> {
    let formats = convert_mac_format(mac)?;
    let normalized = formats["plain"].as_str().unwrap_or("");
    let oui = &normalized[..6];
    let url = format!("https://api.macvendors.com/{oui}");
    let output = Command::new("curl.exe")
        .args(["-sS", "--max-time", "6", "-w", "\n%{http_code}", &url])
        .creation_flags(0x08000000)
        .output()
        .await
        .map_err(|error| error.to_string())?;
    let text = String::from_utf8_lossy(&output.stdout);
    let (body, status) = text.rsplit_once('\n').unwrap_or((&text, "0"));
    let found = output.status.success() && status.trim() == "200" && !body.trim().is_empty();
    Ok(
        json!({"mac":formats["colon"],"oui":format!("{}:{}:{}",&oui[0..2],&oui[2..4],&oui[4..6]),"vendor":if found{body.trim()}else{"未知厂商"},"source":if found{"remote"}else{"unknown"},"formats":formats}),
    )
}

#[tauri::command]
pub async fn list_arp_table() -> Result<Value, String> {
    let output = Command::new("arp.exe")
        .arg("-a")
        .creation_flags(0x08000000)
        .output()
        .await
        .map_err(|error| error.to_string())?;
    let text = String::from_utf8_lossy(&output.stdout);
    let rows = text.lines().filter_map(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() >= 3 && fields[0].parse::<std::net::Ipv4Addr>().is_ok() && fields[1].contains('-') {
            Some(json!({"ip": fields[0], "mac": fields[1].to_uppercase().replace('-', ":"), "type": fields[2], "vendor": "未知"}))
        } else { None }
    }).collect::<Vec<_>>();
    Ok(json!(rows))
}

#[tauri::command]
pub async fn scan_wifi_networks(interface: Option<String>) -> Result<Value, String> {
    let _permit = WIFI_SCAN.try_acquire().map_err(|_| "WiFi 扫描正在进行，请稍后重试")?;
    let lines = wifi_lines(&wifi_scan_arguments(interface.as_deref())?).await?;
    parse_wifi_networks(&lines)
}

fn parse_wifi_networks(lines: &[String]) -> Result<Value, String> {
    let mut aps = Vec::<Value>::new();
    let mut interfaces = Vec::new();
    let mut interface_name = String::new();
    let mut ssid = String::new();
    let mut auth = String::new();
    for line in lines {
        let t = line.trim();
        if t.starts_with("Interface name") || t.starts_with("接口名称") || t.starts_with("界面名称") {
            interface_name = after_colon(t).to_string();
            interfaces.push(json!({"name":interface_name,"kind":"wifi"}));
            ssid.clear();
            auth.clear();
        } else if t.starts_with("SSID ") && !t.starts_with("BSSID") {
            ssid = after_colon(t).to_string();
            auth.clear();
        } else if t.starts_with("Authentication") || t.starts_with("身份验证") {
            auth = after_colon(t).to_string();
        } else if t.starts_with("BSSID ") {
            aps.push(json!({"ssid":ssid,"bssid":after_colon(t),"interface":interface_name,"auth":auth,"signal_percent":0,"signal_dbm":-100,"quality":"很差","channel":0,"band":"2.4G"}));
        } else if t.starts_with("Signal") || t.starts_with("信号") {
            if let Some(last) = aps.last_mut() {
                let pct = after_colon(t)
                    .trim_end_matches('%')
                    .parse::<i64>()
                    .unwrap_or(0)
                    .clamp(0, 100);
                last["signal_percent"] = json!(pct);
                last["signal_dbm"] = json!(pct / 2 - 100);
                last["quality"] = json!(if pct >= 80 {
                    "极佳"
                } else if pct >= 65 {
                    "良好"
                } else if pct >= 50 {
                    "一般"
                } else if pct >= 30 {
                    "较差"
                } else {
                    "很差"
                });
            }
        } else if t.starts_with("Channel") || t.starts_with("信道") {
            if let Some(last) = aps.last_mut() {
                let channel = after_colon(t).parse::<u64>().unwrap_or(0);
                last["channel"] = json!(channel);
                last["band"] = json!(if channel <= 14 { "2.4G" } else { "5G" });
            }
        }
    }
    if interfaces.is_empty() && aps.is_empty() {
        let message = lines.iter().map(|line| line.trim()).filter(|line| !line.is_empty()).take(8).collect::<Vec<_>>().join("\n");
        return Err(if message.is_empty() { "WiFi 查询未返回数据".into() } else { message });
    }
    let mut weights = std::collections::BTreeMap::<u64, u64>::new();
    for ap in &aps {
        let c = ap["channel"].as_u64().unwrap_or(0);
        *weights.entry(c).or_default() += ap["signal_percent"].as_u64().unwrap_or(0);
    }
    let channel_stats=weights.iter().map(|(channel,weight)|json!({"channel":channel,"weight":weight,"band":if *channel<=14{"2.4G"}else{"5G"}})).collect::<Vec<_>>();
    let mut candidates = (1..=13)
        .map(|channel| (channel, *weights.get(&channel).unwrap_or(&0)))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|v| v.1);
    Ok(
        json!({"interfaces":interfaces,"aps":aps,"channel_stats":channel_stats,"recommend_24g":candidates.into_iter().take(3).map(|v|v.0).collect::<Vec<_>>(),"message":format!("扫描完成，发现 {} 个接入点",aps.len())}),
    )
}

#[tauri::command]
pub async fn list_wifi_interfaces() -> Result<Value, String> {
    let lines = wifi_lines(&["wlan".into(), "show".into(), "interfaces".into()]).await?;
    Ok(json!(parse_wifi_interfaces(&lines)))
}

fn parse_wifi_interfaces(lines: &[String]) -> Vec<Value> {
    let mut interfaces = Vec::<Value>::new();
    for line in lines {
        let Some((key, value)) = line.trim().split_once(':') else { continue; };
        let value = value.trim();
        match key.trim() {
            "Name" | "名称" | "名字" => interfaces.push(json!({"name":value,"kind":"wifi","ssid":""})),
            "Description" | "描述" => { if let Some(item) = interfaces.last_mut() { item["description"] = json!(value); } }
            "SSID" => { if let Some(item) = interfaces.last_mut() { item["ssid"] = json!(value); } }
            _ => {}
        }
    }
    interfaces
}

fn list_wifi_passwords_blocking() -> Result<Value, String> {
    let lines = netsh_lines("wlan show profiles")?;
    let mut names = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.starts_with("All User Profile") || t.starts_with("所有用户配置文件") {
            let name = after_colon(t);
            if !name.is_empty() {
                names.push(name.to_string())
            }
        }
    }
    let mut profiles = Vec::new();
    let mut auth_counts = std::collections::BTreeMap::<String, u64>::new();
    let mut status_counts = std::collections::BTreeMap::<String, u64>::new();
    for name in names {
        let arg = format!(
            "wlan show profile name=\"{}\" key=clear",
            name.replace('"', "")
        );
        let detail = netsh_lines(&arg)?;
        let mut password = String::new();
        let mut auth = String::new();
        for line in detail {
            let t = line.trim();
            if t.starts_with("Key Content") || t.starts_with("关键内容") {
                password = after_colon(t).to_string()
            } else if t.starts_with("Authentication") || t.starts_with("身份验证") {
                auth = after_colon(t).to_string()
            }
        }
        let status = if password.is_empty() { "no_key" } else { "ok" };
        *auth_counts
            .entry(if auth.is_empty() {
                "未知".into()
            } else {
                auth.clone()
            })
            .or_default() += 1;
        *status_counts.entry(status.into()).or_default() += 1;
        profiles.push(json!({"ssid":name,"auth":auth,"password":password,"status":status}));
    }
    Ok(
        json!({"profiles":profiles,"auth_stats":auth_counts.into_iter().collect::<Vec<_>>(),"status_stats":status_counts.into_iter().collect::<Vec<_>>(),"message":format!("读取到 {} 个 WiFi 配置",profiles.len())}),
    )
}


#[tauri::command]
pub async fn list_wifi_passwords() -> Result<Value, String> {
    tokio::task::spawn_blocking(list_wifi_passwords_blocking)
        .await
        .map_err(|error| format!("读取 WiFi 配置任务失败：{error}"))?
}

#[cfg(test)]
mod wifi_tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> { text.lines().map(str::to_string).collect() }

    #[test]
    fn interfaces_include_disconnected_adapters_without_ip_queries() {
        let result = parse_wifi_interfaces(&lines("Name : Wi-Fi\nDescription : Adapter A\nState : disconnected\n名称 : 无线网络\n描述 : Adapter B\nSSID : Network B"));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["name"], "Wi-Fi");
        assert_eq!(result[0]["ssid"], "");
        assert_eq!(result[1]["name"], "无线网络");
        assert_eq!(result[1]["ssid"], "Network B");
        assert!(parse_wifi_interfaces(&lines("There is no wireless interface on the system.")).is_empty());
    }

    #[test]
    fn scan_parser_handles_localized_output_and_no_networks() {
        let parsed = parse_wifi_networks(&lines("接口名称 : 无线网络\nSSID 1 : Test\n身份验证 : WPA2\nBSSID 1 : 00:11:22:33:44:55\n信号 : 80%\n信道 : 6")).unwrap();
        assert_eq!(parsed["interfaces"][0]["name"], "无线网络");
        assert_eq!(parsed["aps"][0]["interface"], "无线网络");
        assert_eq!(parsed["aps"][0]["signal_dbm"], -60);
        assert_eq!(parsed["aps"][0]["channel"], 6);
        let empty = parse_wifi_networks(&lines("Interface name : Wi-Fi\nThere are 0 networks currently visible.")).unwrap();
        assert_eq!(empty["aps"], json!([]));
        assert!(parse_wifi_networks(&lines("Access is denied. Enable location permission.")).unwrap_err().contains("Access is denied"));
    }

    #[test]
    fn interface_argument_is_literal_and_control_characters_are_rejected() {
        let arguments = wifi_scan_arguments(Some("Wi-Fi $(test) & name")).unwrap();
        assert_eq!(arguments, vec!["wlan", "show", "networks", "mode=bssid", "interface=Wi-Fi $(test) & name"]);
        assert_eq!(wifi_scan_arguments(None).unwrap().len(), 4);
        assert!(wifi_scan_arguments(Some("Wi-Fi\nsecond command")).is_err());
        assert_eq!(decode_netsh_output("SSID : 无线网络".as_bytes()).unwrap(), "SSID : 无线网络");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn hanging_wifi_command_times_out_without_blocking_the_runtime() {
        let mut command = Command::new("powershell.exe");
        command.args(["-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 30"]);
        let started = Instant::now();
        let operation = wifi_command_output(command, Duration::from_millis(500));
        let heartbeat = async {
            tokio::time::sleep(Duration::from_millis(40)).await;
            assert!(started.elapsed() < Duration::from_millis(400));
        };
        let (result, _) = tokio::join!(operation, heartbeat);
        assert!(result.unwrap_err().contains("超时"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
