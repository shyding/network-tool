use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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
        .map_err(|error| format!("无法启动 PowerShell：{error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if message.is_empty() {
            "防火墙操作失败".into()
        } else {
            message
        })
    }
}

fn ps_json(script: &str) -> Result<Value, String> {
    let text = ps(script)?;
    serde_json::from_str(&text).map_err(|error| format!("解析防火墙状态失败：{error}"))
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub fn elevated() -> bool {
    ps("[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent() | ForEach-Object { $_.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) }")
        .is_ok_and(|value| value.eq_ignore_ascii_case("true"))
}

fn firewall_snapshot_blocking() -> Result<Value, String> {
    let mut value = ps_json(
        r#"$profiles = @(Get-NetFirewallProfile | ForEach-Object { [pscustomobject]@{ key=$_.Name.ToString().ToLowerInvariant(); enabled=[bool]$_.Enabled } })
        $portMap=@{}; @(Get-NetFirewallPortFilter -ErrorAction SilentlyContinue) | ForEach-Object {$portMap[$_.InstanceID]=$_}
        $rules = @(Get-NetFirewallRule | ForEach-Object {
          $r=$_; $pf=$portMap[$r.InstanceID]
          [pscustomobject]@{ name=[string]$r.DisplayName; direction=[string]$r.Direction; action=[string]$r.Action; protocol=if($pf){[string]$pf.Protocol}else{'Any'}; local_port=if($pf){[string]$pf.LocalPort}else{''}; enabled=([string]$r.Enabled -eq 'True'); profile=[string]$r.Profile; toolkit=([string]$r.DisplayName -like '网络测试工具箱*') }
        })
        [pscustomobject]@{ profiles=$profiles; rules=$rules } | ConvertTo-Json -Depth 5 -Compress"#,
    )?;
    value["elevated"] = json!(elevated());
    Ok(value)
}

#[tauri::command]
pub async fn firewall_snapshot() -> Result<Value, String> {
    tokio::task::spawn_blocking(firewall_snapshot_blocking)
        .await
        .map_err(|error| format!("读取防火墙状态任务失败：{error}"))?
}

#[tauri::command]
pub fn firewall_add_port_rule(
    name: String,
    port: String,
    protocol: String,
    direction: String,
    action: String,
    profile: String,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("规则名称不能为空".into());
    }
    if port.parse::<u16>().is_err() {
        return Err("端口必须是 1-65535".into());
    }
    let direction = if direction.eq_ignore_ascii_case("out") {
        "Outbound"
    } else {
        "Inbound"
    };
    let action = if action.eq_ignore_ascii_case("block") {
        "Block"
    } else {
        "Allow"
    };
    let profile = match profile.to_ascii_lowercase().as_str() {
        "domain" => "Domain",
        "private" => "Private",
        "public" => "Public",
        _ => "Any",
    };
    let display = if name.starts_with("网络测试工具箱") {
        name
    } else {
        format!("网络测试工具箱 - {name}")
    };
    ps(&format!("New-NetFirewallRule -DisplayName {} -Direction {direction} -Action {action} -Protocol {} -LocalPort {} -Profile {profile} | Out-Null; '已添加防火墙规则：' + {}", quote(&display), quote(&protocol), quote(&port), quote(&display)))
}

#[tauri::command]
pub fn firewall_set_all(enabled: bool) -> Result<String, String> {
    ps(&format!(
        "Set-NetFirewallProfile -Profile Domain,Private,Public -Enabled {}; '已{}全部防火墙配置'",
        if enabled { "True" } else { "False" },
        if enabled { "开启" } else { "关闭" }
    ))
}

#[tauri::command]
pub fn firewall_set_profile(profile: String, enabled: bool) -> Result<String, String> {
    let profile = match profile.to_ascii_lowercase().as_str() {
        "domain" => "Domain",
        "private" => "Private",
        "public" => "Public",
        _ => return Err("未知防火墙配置文件".into()),
    };
    ps(&format!(
        "Set-NetFirewallProfile -Profile {profile} -Enabled {}; '已更新 {profile} 防火墙'",
        if enabled { "True" } else { "False" }
    ))
}

#[tauri::command]
pub fn firewall_set_rule_enabled(name: String, enabled: bool) -> Result<String, String> {
    ps(&format!(
        "Set-NetFirewallRule -DisplayName {} -Enabled {}; '规则已{}：' + {}",
        quote(&name),
        if enabled { "True" } else { "False" },
        if enabled { "启用" } else { "禁用" },
        quote(&name)
    ))
}

#[tauri::command]
pub fn firewall_delete_rule(name: String) -> Result<String, String> {
    ps(&format!(
        "Remove-NetFirewallRule -DisplayName {}; '规则已删除：' + {}",
        quote(&name),
        quote(&name)
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_firewall_without_mutating_it() {
        let value = super::firewall_snapshot_blocking().expect("firewall snapshot should succeed");
        assert_eq!(value["profiles"].as_array().map(Vec::len), Some(3));
        assert!(value["rules"].is_array());
        assert!(value["elevated"].is_boolean());
    }
}
