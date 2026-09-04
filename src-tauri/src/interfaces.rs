use ipnet::Ipv4Net;
use serde_json::{json, Value};
use std::net::Ipv4Addr;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn powershell_json(script: &str) -> Result<Value, String> {
    let wrapped = format!(
        "$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); {script}"
    );
    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &wrapped,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("无法启动 PowerShell：{error}"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if message.is_empty() {
            "读取 Windows 网络配置失败".into()
        } else {
            message
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("解析网络配置失败：{error}"))
}

const INTERFACE_CACHE_TTL: Duration = Duration::from_secs(15);
static INTERFACE_CACHE: OnceLock<Mutex<Option<(Instant, Vec<Value>)>>> = OnceLock::new();

fn query_all_interfaces() -> Result<Vec<Value>, String> {
    let raw = powershell_json(
        r#"@(
          Get-NetIPConfiguration | Where-Object { $_.IPv4Address } | ForEach-Object {
            $cfg = $_
            $addr = @($cfg.IPv4Address)[0]
            $adapter = Get-NetAdapter -InterfaceIndex $cfg.InterfaceIndex -ErrorAction SilentlyContinue
            $ipif = Get-NetIPInterface -InterfaceIndex $cfg.InterfaceIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue
            [pscustomobject]@{
              id = [string]$cfg.InterfaceIndex
              name = [string]$cfg.InterfaceAlias
              description = [string]$cfg.InterfaceDescription
              ipv4 = [string]$addr.IPAddress
              prefix = [int]$addr.PrefixLength
              gateway = if ($cfg.IPv4DefaultGateway) { [string]$cfg.IPv4DefaultGateway.NextHop } else { $null }
              dns = @($cfg.DNSServer.ServerAddresses)
              mac = if ($adapter) { [string]$adapter.MacAddress } else { $null }
              status = if ($adapter) { [string]$adapter.Status } else { 'Unknown' }
              speed = if ($adapter) { [uint64]$adapter.ReceiveLinkSpeed } else { 0 }
              dhcp = if ($ipif) { [string]$ipif.Dhcp -eq 'Enabled' } else { $false }
            }
          }
        ) | ConvertTo-Json -Depth 5 -Compress"#,
    )?;
    let source = raw.as_array().cloned().unwrap_or_default();
    let primary_index = source.iter().position(|item| {
        item["gateway"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
            && item["status"].as_str() == Some("Up")
    });
    Ok(source.into_iter().enumerate().map(|(index, item)| {
        let ipv4 = item["ipv4"].as_str().unwrap_or("0.0.0.0");
        let prefix = item["prefix"].as_u64().unwrap_or(24).min(32) as u8;
        let parsed_net = ipv4.parse::<Ipv4Addr>().ok().and_then(|address| Ipv4Net::new(address, prefix).ok());
        let network = parsed_net.map(|net| net.to_string()).unwrap_or_else(|| format!("{ipv4}/{prefix}"));
        let netmask = parsed_net.map(|net| net.netmask().to_string()).unwrap_or_default();
        let name = item["name"].as_str().unwrap_or("网络接口");
        let description = item["description"].as_str().unwrap_or("");
        let kind = if name.to_ascii_lowercase().contains("wi-fi") || name.contains("无线") || description.to_ascii_lowercase().contains("wireless") { "wifi" }
            else if ipv4.starts_with("127.") { "loopback" } else { "ethernet" };
        json!({
            "id": item["id"], "key": item["id"], "name": name, "description": description, "device": if description.is_empty(){name}else{description},
            "label": format!("{name} · {ipv4}"), "value": item["id"],
            "kind": kind, "kind_label": if kind == "wifi" { "无线" } else if kind == "loopback" { "环回" } else { "以太网" },
            "ip": ipv4, "ipv4": ipv4, "prefix": prefix, "netmask": netmask,
            "network": network, "suggested_range": network, "gateway": item["gateway"], "dns": item["dns"],
            "mac": item["mac"], "is_up": item["status"].as_str() == Some("Up"),
            "is_lan_primary": primary_index == Some(index), "dhcp": item["dhcp"], "speed": item["speed"]
        })
    }).collect())
}

pub fn all_interfaces() -> Result<Vec<Value>, String> {
    let cache = INTERFACE_CACHE.get_or_init(|| Mutex::new(None));
    let mut cached = cache
        .lock()
        .map_err(|_| "网卡信息缓存锁已损坏".to_string())?;
    if let Some((created_at, interfaces)) = cached.as_ref() {
        if created_at.elapsed() < INTERFACE_CACHE_TTL {
            return Ok(interfaces.clone());
        }
    }

    let interfaces = query_all_interfaces()?;
    *cached = Some((Instant::now(), interfaces.clone()));
    Ok(interfaces)
}

fn interfaces_value() -> Result<Value, String> {
    Ok(Value::Array(all_interfaces()?))
}

async fn interfaces_value_async() -> Result<Value, String> {
    tokio::task::spawn_blocking(interfaces_value)
        .await
        .map_err(|error| format!("读取网卡信息任务失败：{error}"))?
}

#[tauri::command]
pub async fn list_segment_ifaces() -> Result<Value, String> {
    interfaces_value_async().await
}
#[tauri::command]
pub async fn list_lan_ifaces() -> Result<Value, String> {
    interfaces_value_async().await
}
#[tauri::command]
pub async fn list_conflict_ifaces() -> Result<Value, String> {
    interfaces_value_async().await
}
#[tauri::command]
pub async fn list_dhcp_ifaces() -> Result<Value, String> {
    interfaces_value_async().await
}
#[tauri::command]
pub async fn list_loop_interfaces() -> Result<Value, String> {
    interfaces_value_async().await
}
#[tauri::command]
pub async fn list_traffic_ifaces() -> Result<Value, String> {
    interfaces_value_async().await
}
#[tauri::command]
pub async fn list_lldp_ifaces() -> Result<Value, String> {
    interfaces_value_async().await
}
#[tauri::command]
pub async fn local_list_ip_ifaces() -> Result<Value, String> {
    interfaces_value_async().await
}

fn local_ip_info_blocking() -> Result<Value, String> {
    let interfaces = all_interfaces()?;
    let entries = interfaces
        .iter()
        .map(|item| {
            let ip = item["ipv4"].as_str().unwrap_or("");
            let scope = if ip.starts_with("10.")
                || ip.starts_with("192.168.")
                || ip.starts_with("172.")
                || ip.starts_with("127.")
            {
                "私有"
            } else {
                "公网"
            };
            json!({"ip": ip, "iface": item["name"], "kind": "IPv4", "scope": scope})
        })
        .collect::<Vec<_>>();
    let private_count = entries
        .iter()
        .filter(|entry| entry["scope"] == "私有")
        .count();
    let primary_ip = interfaces
        .iter()
        .find(|item| item["is_lan_primary"] == true)
        .or_else(|| interfaces.first())
        .and_then(|item| item["ipv4"].as_str())
        .unwrap_or("127.0.0.1");
    Ok(json!({
        "hostname": sysinfo::System::host_name().unwrap_or_else(|| "Windows".into()),
        "primary_ip": primary_ip, "entries": entries, "private_count": private_count,
        "public_count": interfaces.len().saturating_sub(private_count), "linklocal_count": 0
    }))
}

#[tauri::command]
pub async fn get_local_ip_info() -> Result<Value, String> {
    tokio::task::spawn_blocking(local_ip_info_blocking)
        .await
        .map_err(|error| format!("读取本机 IP 任务失败：{error}"))?
}

#[cfg(test)]
mod tests {
    #[test]
    fn enumerates_real_windows_interfaces() {
        let items = super::all_interfaces().expect("Windows interface query should succeed");
        assert!(!items.is_empty());
        assert!(items.iter().all(|item| item["name"].is_string()
            && item["ipv4"].is_string()
            && item["network"].is_string()));
        assert!(items.iter().any(|item| item["is_lan_primary"] == true));
    }
}
