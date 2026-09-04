use serde_json::{json, Value};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

#[tauri::command]
pub fn skill_list() -> Value {
    json!([
        {"name":"dns_lookup","display":"DNS 查询","description":"解析域名的 IPv4/IPv6 地址"},
        {"name":"tcp_check","display":"TCP 端口检测","description":"检测指定主机端口是否可连接"},
        {"name":"ping","display":"Ping 测试","description":"发送 ICMP 回显请求"},
        {"name":"local_network","display":"本机网络信息","description":"读取网卡、地址、网关与 DNS"}
    ])
}
#[tauri::command]
pub fn skill_prompt() -> String {
    r#"你是网络运维助手。需要调用本机工具时只输出 JSON：{"skill":"dns_lookup|tcp_check|ping|local_network","params":{...}}；无需工具时输出 {"skill":"none","answer":"中文回答"}。dns_lookup 参数 domain；tcp_check 参数 host、port；ping 参数 host；local_network 无参数。收到工具结果后给出简洁、可操作的中文结论。"#.into()
}

#[tauri::command]
pub async fn skill_execute(name: String, params: Value) -> Result<Value, String> {
    match name.as_str() {
        "dns_lookup" => {
            let domain = params["domain"].as_str().ok_or("缺少 domain")?.to_string();
            let data = crate::diagnostics::tools_dns_lookup(domain.clone()).await?;
            Ok(json!({"display":format!("DNS 查询 {domain}"),"data":data}))
        }
        "tcp_check" => {
            let host = params["host"].as_str().ok_or("缺少 host")?.to_string();
            let port = params["port"].as_u64().ok_or("缺少 port")? as u16;
            let data = crate::diagnostics::tools_telnet_test(host.clone(), port, 5.0).await;
            Ok(json!({"display":format!("TCP 检测 {host}:{port}"),"data":data}))
        }
        "ping" => {
            let host = params["host"].as_str().ok_or("缺少 host")?;
            let out = tokio::process::Command::new("ping.exe")
                .args(["-n", "4", "-w", "2000", host])
                .creation_flags(0x08000000)
                .output()
                .await
                .map_err(|e| e.to_string())?;
            Ok(
                json!({"display":format!("Ping {host}"),"data":{"ok":out.status.success(),"output":String::from_utf8_lossy(&out.stdout)}}),
            )
        }
        "local_network" => {
            Ok(json!({"display":"本机网络信息","data":crate::interfaces::all_interfaces()?}))
        }
        _ => Err(format!("未知 Skill：{name}")),
    }
}

fn webhook(url: String, body: Value) -> Result<bool, String> {
    if !url.starts_with("https://") {
        return Err("Webhook 必须使用 HTTPS".into());
    }
    let mut child = Command::new("curl.exe")
        .args([
            "-sS",
            "--fail",
            "--max-time",
            "15",
            "-H",
            "Content-Type: application/json",
            "--data-binary",
            "@-",
            &url,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("无法写入 Webhook 请求")?
        .write_all(body.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(true)
    } else {
        Err(format!(
            "Webhook 发送失败：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
#[tauri::command]
pub fn send_feishu_webhook(url: String, text: String) -> Result<bool, String> {
    webhook(url, json!({"msg_type":"text","content":{"text":text}}))
}
#[tauri::command]
pub fn send_wecom_webhook(url: String, text: String) -> Result<bool, String> {
    webhook(url, json!({"msgtype":"text","text":{"content":text}}))
}
