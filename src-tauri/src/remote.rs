use serde_json::{json, Value};
use std::net::UdpSocket;
use std::os::windows::process::CommandExt;
use std::process::Command;

#[tauri::command]
pub fn send_wake_on_lan(mac: String, broadcast: String, port: u16) -> Result<bool, String> {
    let bytes = mac
        .split(|c| c == ':' || c == '-')
        .map(|part| u8::from_str_radix(part, 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "MAC 地址格式错误")?;
    if bytes.len() != 6 {
        return Err("MAC 地址应包含 6 个字节".into());
    }
    let mut packet = Vec::with_capacity(102);
    packet.extend_from_slice(&[0xff; 6]);
    for _ in 0..16 {
        packet.extend_from_slice(&bytes)
    }
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| error.to_string())?;
    socket
        .set_broadcast(true)
        .map_err(|error| error.to_string())?;
    socket
        .send_to(&packet, format!("{broadcast}:{port}"))
        .map_err(|error| format!("发送魔术包失败：{error}"))?;
    if port != 7 {
        let _ = socket.send_to(&packet, format!("{broadcast}:7"));
    }
    Ok(true)
}

#[tauri::command]
pub fn launch_rdp(
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    size: String,
) -> Result<Value, String> {
    if host.trim().is_empty() {
        return Err("主机不能为空".into());
    }
    if let (Some(user), Some(pass)) = (
        username.as_deref().filter(|s| !s.is_empty()),
        password.as_deref().filter(|s| !s.is_empty()),
    ) {
        let status = Command::new("cmdkey.exe")
            .args([
                format!("/generic:TERMSRV/{}", host),
                format!("/user:{user}"),
                format!("/pass:{pass}"),
            ])
            .creation_flags(0x08000000)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err("保存远程桌面凭据失败".into());
        }
    }
    let mut command = Command::new("mstsc.exe");
    command.arg(format!("/v:{host}:{port}"));
    if size == "fullscreen" {
        command.arg("/f");
    } else if let Some((w, h)) = size.split_once('x') {
        command.args([format!("/w:{w}"), format!("/h:{h}")]);
    }
    command
        .spawn()
        .map_err(|error| format!("启动远程桌面失败：{error}"))?;
    Ok(json!({"success":true,"message":format!("已启动远程桌面：{host}:{port}")}))
}
