use serde::Deserialize;
use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn ps(script: &str) -> Result<String, String> {
    let wrapped = format!("$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); {script}");
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
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let e = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if e.is_empty() {
            "系统操作失败".into()
        } else {
            e
        })
    }
}

fn ps_json(script: &str) -> Result<Value, String> {
    serde_json::from_str(&ps(script)?).map_err(|error| format!("解析系统状态失败：{error}"))
}

fn local_settings_snapshot_blocking() -> Result<Value, String> {
    let mut value = ps_json(
        r#"
      $nics=@(Get-NetIPConfiguration | Where-Object {$_.IPv4Address} | ForEach-Object {
        $a=Get-NetAdapter -InterfaceIndex $_.InterfaceIndex -ErrorAction SilentlyContinue
        $addr=@($_.IPv4Address)[0]
        [pscustomobject]@{name=$_.InterfaceAlias; ipv4=[string]$addr.IPAddress; kind_label=if($a.MediaType -match '802.11'){'无线'}else{'以太网'}}
      })
      $dns=@(Get-DnsClientServerAddress -AddressFamily IPv4 | Where-Object {$_.ServerAddresses} | ForEach-Object {$_.ServerAddresses} | Select-Object -Unique)
      $fw=@(Get-NetFirewallProfile | ForEach-Object {[pscustomobject]@{name=$_.Name.ToString();enabled=[bool]$_.Enabled}})
      $proxy=(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' -ErrorAction SilentlyContinue)
      $rdp=(Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server\WinStations\RDP-Tcp' -Name PortNumber -ErrorAction SilentlyContinue).PortNumber
      [pscustomobject]@{hostname=$env:COMPUTERNAME;elevated=$false;rdp_port=if($rdp){[int]$rdp}else{3389};proxy_enabled=([int]$proxy.ProxyEnable -eq 1);proxy_server=[string]$proxy.ProxyServer;dns_servers=$dns;nics=$nics;firewall=$fw} | ConvertTo-Json -Depth 5 -Compress
    "#,
    )?;
    value["elevated"] = json!(crate::firewall::elevated());
    Ok(value)
}

#[tauri::command]
pub async fn local_settings_snapshot() -> Result<Value, String> {
    tokio::task::spawn_blocking(local_settings_snapshot_blocking)
        .await
        .map_err(|error| format!("读取本机设置任务失败：{error}"))?
}

#[tauri::command(async)]
pub fn local_get_ip_config(interface: String) -> Result<Value, String> {
    crate::interfaces::all_interfaces()?.into_iter().find(|item| item["name"] == interface)
        .map(|item| json!({"interface": interface, "dhcp": item["dhcp"], "ip": item["ipv4"], "mask": item["netmask"], "gateway": item["gateway"], "dns": item["dns"]}))
        .ok_or_else(|| "没有找到指定网络接口".into())
}

#[derive(Deserialize)]
pub struct IpConfigRequest {
    interface: String,
    mode: String,
    ip: Option<String>,
    mask: Option<String>,
    gateway: Option<String>,
    dns: Option<String>,
}

#[tauri::command(async)]
pub fn local_apply_ip_config(req: IpConfigRequest) -> Result<String, String> {
    let iface = quote(&req.interface);
    if req.mode.eq_ignore_ascii_case("dhcp") {
        ps(&format!("Set-NetIPInterface -InterfaceAlias {iface} -Dhcp Enabled; Set-DnsClientServerAddress -InterfaceAlias {iface} -ResetServerAddresses; '已将网卡设置为 DHCP：' + {iface}"))
    } else {
        let ip = req.ip.as_deref().ok_or("静态配置缺少 IP 地址")?;
        let mask = req.mask.as_deref().ok_or("静态配置缺少子网掩码")?;
        let prefix = mask_to_prefix(mask)?;
        let gateway = req.gateway.as_deref().unwrap_or("");
        let dns = req.dns.as_deref().unwrap_or("");
        let gateway_part = if gateway.is_empty() {
            String::new()
        } else {
            format!(" -DefaultGateway {}", quote(gateway))
        };
        let dns_part = if dns.trim().is_empty() {
            String::new()
        } else {
            let servers = dns
                .split(',')
                .map(|s| quote(s.trim()))
                .collect::<Vec<_>>()
                .join(",");
            format!("; Set-DnsClientServerAddress -InterfaceAlias {iface} -ServerAddresses @({servers})")
        };
        ps(&format!("Get-NetIPAddress -InterfaceAlias {iface} -AddressFamily IPv4 -ErrorAction SilentlyContinue | Remove-NetIPAddress -Confirm:$false; New-NetIPAddress -InterfaceAlias {iface} -IPAddress {} -PrefixLength {prefix}{gateway_part} | Out-Null{dns_part}; '已应用静态 IPv4 配置：' + {iface}",quote(ip)))
    }
}

fn mask_to_prefix(mask: &str) -> Result<u32, String> {
    let mut bits = 0u32;
    let mut zero_seen = false;
    for octet in mask.split('.') {
        let n = octet.parse::<u8>().map_err(|_| "子网掩码格式错误")?;
        for bit in (0..8).rev() {
            if n & (1 << bit) != 0 {
                if zero_seen {
                    return Err("子网掩码不连续".into());
                }
                bits += 1
            } else {
                zero_seen = true
            }
        }
    }
    if mask.split('.').count() != 4 {
        Err("子网掩码格式错误".into())
    } else {
        Ok(bits)
    }
}

#[tauri::command(async)]
pub fn local_open_tool(tool: String) -> Result<String, String> {
    let (program, args): (&str, &[&str]) = match tool.as_str() {
        "cmd" => ("cmd.exe", &[]),
        "powershell" => ("powershell.exe", &[]),
        "ncpa" => ("control.exe", &["ncpa.cpl"]),
        "netsettings" => ("cmd.exe", &["/c", "start", "ms-settings:network-status"]),
        "firewall" => ("control.exe", &["firewall.cpl"]),
        "wf" => ("wf.msc", &[]),
        "device" => ("devmgmt.msc", &[]),
        "taskmgr" => ("taskmgr.exe", &[]),
        "msinfo" => ("msinfo32.exe", &[]),
        "regedit" => ("regedit.exe", &[]),
        "eventvwr" => ("eventvwr.msc", &[]),
        "diskmgmt" => ("diskmgmt.msc", &[]),
        "services" => ("services.msc", &[]),
        "resmon" => ("resmon.exe", &[]),
        "perfmon" => ("perfmon.exe", &[]),
        "compmgmt" => ("compmgmt.msc", &[]),
        "control" => ("control.exe", &[]),
        "appwiz" => ("control.exe", &["appwiz.cpl"]),
        "mstsc" => ("mstsc.exe", &[]),
        "netdiag" => ("msdt.exe", &["-id", "NetworkDiagnosticsNetworkAdapter"]),
        "admin_cmd" => return open_elevated("cmd.exe"),
        "admin_powershell" => return open_elevated("powershell.exe"),
        _ => return Err(format!("未知系统工具：{tool}")),
    };
    Command::new(program)
        .args(args)
        .spawn()
        .map_err(|error| format!("无法打开系统工具：{error}"))?;
    Ok(format!("已打开：{tool}"))
}

fn open_elevated(program: &str) -> Result<String, String> {
    ps(&format!(
        "Start-Process {} -Verb RunAs; '已请求管理员权限打开工具'",
        quote(program)
    ))
}

#[tauri::command(async)]
pub fn local_run_action(action: String, arg: Option<String>) -> Result<String, String> {
    match action.as_str() {
        "flush_dns" => ps("Clear-DnsClientCache; 'DNS 缓存已清理'"),
        "dhcp_renew" => {
            ps("ipconfig /release | Out-Null; ipconfig /renew | Out-Null; 'DHCP 地址已重新获取'")
        }
        "fw_on" => crate::firewall::firewall_set_all(true),
        "fw_off" => crate::firewall::firewall_set_all(false),
        "fw_status" => ps("netsh advfirewall show allprofiles state"),
        "winsock" => ps("netsh winsock reset | Out-Null; 'Winsock 已重置，重启后生效'"),
        "tcpip" => ps("netsh int ip reset | Out-Null; 'TCP/IP 已重置，重启后生效'"),
        "clear_proxy" => ps(
            r#"Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' ProxyEnable 0; netsh winhttp reset proxy | Out-Null; '系统代理已清除'"#,
        ),
        "set_rdp_port" | "restore_rdp_port" => {
            let port = if action == "restore_rdp_port" {
                3389
            } else {
                arg.as_deref()
                    .ok_or("缺少 RDP 端口")?
                    .parse::<u16>()
                    .map_err(|_| "RDP 端口无效")?
            };
            ps(&format!(
                r#"Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server\WinStations\RDP-Tcp' -Name PortNumber -Type DWord -Value {port}; 'RDP 端口已设置为 {port}，重启远程桌面服务或计算机后生效'"#
            ))
        }
        _ => Err(format!("未知本机操作：{action}")),
    }
}

#[tauri::command(async)]
pub fn local_run_repair(options: Vec<String>) -> Result<String, String> {
    let mut done = Vec::new();
    for option in options {
        match option.as_str() {
            "winsock" => {
                local_run_action("winsock".into(), None)?;
            }
            "tcpip" => {
                local_run_action("tcpip".into(), None)?;
            }
            "dns" => {
                local_run_action("flush_dns".into(), None)?;
            }
            "dhcp" => {
                local_run_action("dhcp_renew".into(), None)?;
            }
            "proxy" => {
                local_run_action("clear_proxy".into(), None)?;
            }
            "firewall" => {
                ps("netsh advfirewall reset | Out-Null; 'ok'")?;
            }
            "hosts" => {
                ps(
                    r#"$p=Join-Path $env:SystemRoot 'System32\drivers\etc\hosts'; Set-Content -LiteralPath $p -Encoding ASCII -Value "127.0.0.1 localhost`r`n::1 localhost"; 'ok'"#,
                )?;
            }
            _ => return Err(format!("未知修复项目：{option}")),
        };
        done.push(option);
    }
    Ok(format!(
        "修复已执行：{}。部分项目需重启 Windows 后完全生效。",
        done.join("、")
    ))
}

#[tauri::command(async)]
pub fn write_text_file(path: String, content: String) -> Result<bool, String> {
    std::fs::write(path, content)
        .map(|_| true)
        .map_err(|error| format!("写入文件失败：{error}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_local_settings_snapshot() {
        let value =
            super::local_settings_snapshot_blocking().expect("local settings snapshot should succeed");
        assert!(value["hostname"].is_string());
        assert!(value["nics"].is_array());
        assert!(value["dns_servers"].is_array());
        assert!(value["rdp_port"].is_number());
    }
}
