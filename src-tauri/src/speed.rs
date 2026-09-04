use serde_json::json;
use std::os::windows::process::CommandExt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct SpeedState {
    running: Arc<AtomicBool>,
}
#[tauri::command]
pub fn stop_speed_test(state: State<'_, SpeedState>) -> bool {
    state.running.store(false, Ordering::Relaxed);
    true
}
#[tauri::command]
pub fn start_speed_test(
    app: AppHandle,
    state: State<'_, SpeedState>,
    server: String,
) -> Result<bool, String> {
    state.running.store(false, Ordering::Relaxed);
    let running = state.running.clone();
    running.store(true, Ordering::Relaxed);
    std::thread::spawn(move || {
        let url = "https://speed.cloudflare.com/__down?bytes=25000000";
        let _ = app.emit(
            "speedtest:progress",
            json!({"progress":5,"message":"正在连接测速节点…","mbps":0}),
        );
        let started = Instant::now();
        let output = std::process::Command::new("curl.exe")
            .args([
                "-L",
                "-sS",
                "--max-time",
                "45",
                "-o",
                "NUL",
                "-w",
                "%{speed_download} %{time_connect}",
                url,
            ])
            .creation_flags(0x08000000)
            .output();
        if !running.load(Ordering::Relaxed) {
            let _ = app.emit(
                "speedtest:complete",
                json!({"ok":false,"error":"用户已停止测速"}),
            );
            return;
        }
        match output {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                let fields = text.split_whitespace().collect::<Vec<_>>();
                let bytes_sec = fields
                    .first()
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let latency = fields
                    .get(1)
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0)
                    * 1000.0;
                let mbps = bytes_sec * 8.0 / 1_000_000.0;
                let elapsed = started.elapsed().as_secs_f64();
                let sample = json!({"t":elapsed,"mbps":mbps});
                let _ = app.emit("speedtest:sample", &sample);
                let _ = app.emit(
                    "speedtest:progress",
                    json!({"progress":55,"message":"下载完成，正在测试上传…","mbps":mbps}),
                );
                let upload_file = std::env::temp_dir()
                    .join(format!("network-toolbox-upload-{}.bin", std::process::id()));
                let upload_mbps = std::fs::File::create(&upload_file)
                    .and_then(|file| file.set_len(16 * 1024 * 1024))
                    .ok()
                    .and_then(|_| {
                        std::process::Command::new("curl.exe")
                            .args([
                                "-L",
                                "-sS",
                                "--max-time",
                                "45",
                                "-o",
                                "NUL",
                                "-w",
                                "%{speed_upload}",
                                "--data-binary",
                                &format!("@{}", upload_file.display()),
                                "https://speed.cloudflare.com/__up",
                            ])
                            .creation_flags(0x08000000)
                            .output()
                            .ok()
                    })
                    .filter(|result| result.status.success())
                    .and_then(|result| {
                        String::from_utf8_lossy(&result.stdout)
                            .trim()
                            .parse::<f64>()
                            .ok()
                    })
                    .map(|bytes| bytes * 8.0 / 1_000_000.0);
                let _ = std::fs::remove_file(&upload_file);
                if !running.load(Ordering::Relaxed) {
                    let _ = app.emit(
                        "speedtest:complete",
                        json!({"ok":false,"error":"用户已停止测速"}),
                    );
                    return;
                }
                if let Some(value) = upload_mbps {
                    let _ = app.emit(
                        "speedtest:sample",
                        json!({"t":started.elapsed().as_secs_f64(),"mbps":value,"direction":"upload"}),
                    );
                }
                let _ = app.emit(
                    "speedtest:progress",
                    json!({"progress":100,"message":"测速完成","mbps":mbps}),
                );
                let grade = if mbps >= 100.0 {
                    "A"
                } else if mbps >= 50.0 {
                    "B"
                } else if mbps >= 20.0 {
                    "C"
                } else {
                    "D"
                };
                let result = json!({"ok":true,"server":server,"download_mbps":mbps,"upload_mbps":upload_mbps,"peak_mbps":mbps.max(upload_mbps.unwrap_or(0.0)),"latency_ms":latency,"jitter_ms":0,"quality":if mbps>=50.0{"优秀"}else if mbps>=20.0{"良好"}else{"一般"},"grade":grade,"logs":[format!("下载速率 {mbps:.2} Mbps"),upload_mbps.map(|v|format!("上传速率 {v:.2} Mbps")).unwrap_or_else(||"上传测速未返回有效结果".into()),format!("连接延迟 {latency:.1} ms")]});
                let _ = app.emit("speedtest:complete", result);
            }
            Ok(out) => {
                let _ = app.emit(
                    "speedtest:complete",
                    json!({"ok":false,"error":String::from_utf8_lossy(&out.stderr)}),
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "speedtest:complete",
                    json!({"ok":false,"error":e.to_string()}),
                );
            }
        }
        running.store(false, Ordering::Relaxed);
    });
    Ok(true)
}
