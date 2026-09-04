use serde_json::{json, Value};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

fn endpoint(base: &str) -> String {
    let b = base.trim_end_matches('/');
    if b.ends_with("/chat/completions") {
        b.into()
    } else if b.ends_with("/v1") {
        format!("{b}/chat/completions")
    } else {
        format!("{b}/v1/chat/completions")
    }
}

#[tauri::command]
pub fn ai_chat(
    api_url: String,
    api_key: String,
    model: String,
    messages: Vec<Value>,
    temperature: f64,
    timeout_secs: u64,
) -> Result<Value, String> {
    if api_url.trim().is_empty() || api_key.trim().is_empty() {
        return Err("AI 接口地址和密钥不能为空".into());
    }
    let body = json!({"model":model,"messages":messages,"temperature":temperature,"stream":false});
    let mut child = Command::new("curl.exe")
        .args([
            "-sS",
            "--max-time",
            &timeout_secs.clamp(5, 600).to_string(),
            "-H",
            "Content-Type: application/json",
            "-H",
            &format!("Authorization: Bearer {api_key}"),
            "--data-binary",
            "@-",
            &endpoint(&api_url),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("无法打开 AI 请求输入")?
        .write_all(body.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "AI 请求失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let raw: Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("AI 接口返回无效 JSON：{e}"))?;
    if let Some(message) = raw["error"]["message"].as_str() {
        return Err(format!("AI 接口错误：{message}"));
    }
    let content = raw["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("AI 响应中没有 choices[0].message.content")?;
    Ok(json!({"content":content,"usage":raw["usage"],"model":raw["model"]}))
}

#[tauri::command]
pub fn ai_analyze_packet_capture(
    state: tauri::State<'_, crate::capture::CaptureState>,
    api_url: String,
    api_key: String,
    model: String,
    custom_prompt: Option<String>,
    timeout_secs: u64,
) -> Result<String, String> {
    let context = crate::capture::ai_context(&state)?;
    let prompt=format!("你是网络流量安全分析师。根据以下抓包摘要分析协议分布、异常通信、扫描迹象和改进建议。不要虚构摘要中不存在的事实。\n\n抓包摘要：\n{}\n\n用户补充：{}",context,custom_prompt.unwrap_or_else(||"无".into()));
    let response = ai_chat(
        api_url,
        api_key,
        model,
        vec![json!({"role":"user","content":prompt})],
        0.2,
        timeout_secs,
    )?;
    Ok(response["content"].as_str().unwrap_or("").to_string())
}
