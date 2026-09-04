use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};
#[derive(Default)]
pub struct SessionTestState {
    running: Arc<AtomicBool>,
}
#[tauri::command]
pub fn stop_session_test(state: State<'_, SessionTestState>) -> bool {
    state.running.store(false, Ordering::Relaxed);
    true
}
#[tauri::command]
pub fn start_session_test(
    app: AppHandle,
    state: State<'_, SessionTestState>,
    server: String,
    delay_ms: u64,
    threads: usize,
    max_conn: usize,
    max_fail: usize,
) -> Result<bool, String> {
    state.running.store(false, Ordering::Relaxed);
    let running = state.running.clone();
    running.store(true, Ordering::Relaxed);
    tauri::async_runtime::spawn(async move {
        let target = if server == "backup" {
            "1.1.1.1:443"
        } else {
            "www.baidu.com:443"
        };
        let started = Instant::now();
        let mut streams = Vec::new();
        let mut fail = 0usize;
        while running.load(Ordering::Relaxed)
            && streams.len() < max_conn.clamp(1, 100_000)
            && fail < max_fail.clamp(1, 10_000)
        {
            let batch = threads.clamp(1, 500).min(max_conn - streams.len());
            let mut set = tokio::task::JoinSet::new();
            for _ in 0..batch {
                set.spawn(tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    tokio::net::TcpStream::connect(target),
                ));
            }
            while let Some(result) = set.join_next().await {
                match result {
                    Ok(Ok(Ok(stream))) => streams.push(stream),
                    _ => fail += 1,
                }
            }
            let elapsed = started.elapsed().as_secs_f64();
            let message = format!("已建立 {} 个连接，失败 {fail}", streams.len());
            let _ = app.emit(
                "session:progress",
                json!({"total_ok":streams.len(),"fail":fail,"elapsed_s":elapsed,"message":message}),
            );
            if delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms.clamp(1, 60_000)))
                    .await;
            }
        }
        let result = json!({"total_ok":streams.len(),"fail":fail,"elapsed_s":started.elapsed().as_secs_f64(),"message":if running.load(Ordering::Relaxed){"会话测试完成"}else{"用户停止测试"}});
        drop(streams);
        running.store(false, Ordering::Relaxed);
        let _ = app.emit("session:complete", result);
    });
    Ok(true)
}
