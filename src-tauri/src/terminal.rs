use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

enum Connection {
    Ssh(ssh2::Channel),
    Telnet(TcpStream),
}
impl Connection {
    fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Ssh(c) => c.write_all(data),
            Self::Telnet(s) => s.write_all(data),
        }
    }
    fn close(&mut self) {
        match self {
            Self::Ssh(c) => {
                let _ = c.close();
            }
            Self::Telnet(s) => {
                let _ = s.shutdown(std::net::Shutdown::Both);
            }
        }
    }
}
#[derive(Default)]
pub struct TerminalState {
    sessions: Mutex<HashMap<String, Arc<Mutex<Connection>>>>,
}
fn id() -> String {
    format!(
        "term-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
fn address(host: &str, port: u16) -> Result<std::net::SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| "无法解析目标主机".into())
}
fn start_reader(app: AppHandle, session_id: String, connection: Arc<Mutex<Connection>>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            let read = {
                let mut guard = match connection.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                match &mut *guard {
                    Connection::Ssh(c) => c.read(&mut buf),
                    Connection::Telnet(s) => s.read(&mut buf),
                }
            };
            match read {
                Ok(0) => break,
                Ok(n) => {
                    let _=app.emit("term:data",json!({"session_id":session_id,"data":String::from_utf8_lossy(&buf[..n]).into_owned()}));
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Err(_) => break,
            }
        }
        let _ = app.emit("term:closed", json!({"session_id":session_id}));
    });
}

#[tauri::command(async)]
pub fn term_session_ssh_open(
    app: AppHandle,
    state: State<'_, TerminalState>,
    host: String,
    port: u16,
    username: String,
    password: String,
) -> Result<Value, String> {
    let tcp = TcpStream::connect_timeout(&address(&host, port)?, Duration::from_secs(10))
        .map_err(|e| format!("SSH 连接失败：{e}"))?;
    tcp.set_read_timeout(Some(Duration::from_millis(100))).ok();
    tcp.set_write_timeout(Some(Duration::from_secs(10))).ok();
    let mut session = ssh2::Session::new().map_err(|e| e.to_string())?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|e| format!("SSH 握手失败：{e}"))?;
    session
        .userauth_password(&username, &password)
        .map_err(|e| format!("SSH 认证失败：{e}"))?;
    if !session.authenticated() {
        return Err("SSH 用户名或密码错误".into());
    }
    let mut channel = session.channel_session().map_err(|e| e.to_string())?;
    channel
        .request_pty("xterm-256color", None, Some((120, 32, 0, 0)))
        .map_err(|e| e.to_string())?;
    channel.shell().map_err(|e| e.to_string())?;
    session.set_blocking(false);
    let sid = id();
    let conn = Arc::new(Mutex::new(Connection::Ssh(channel)));
    state
        .sessions
        .lock()
        .map_err(|_| "终端状态锁异常")?
        .insert(sid.clone(), conn.clone());
    start_reader(app, sid.clone(), conn);
    Ok(json!({"session_id":sid,"target":format!("{username}@{host}:{port}")}))
}

#[tauri::command(async)]
pub fn term_session_telnet_open(
    app: AppHandle,
    state: State<'_, TerminalState>,
    host: String,
    port: u16,
) -> Result<Value, String> {
    let stream = TcpStream::connect_timeout(&address(&host, port)?, Duration::from_secs(10))
        .map_err(|e| format!("Telnet 连接失败：{e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok();
    let sid = id();
    let conn = Arc::new(Mutex::new(Connection::Telnet(stream)));
    state
        .sessions
        .lock()
        .map_err(|_| "终端状态锁异常")?
        .insert(sid.clone(), conn.clone());
    start_reader(app, sid.clone(), conn);
    Ok(json!({"session_id":sid,"target":format!("{host}:{port}")}))
}

#[tauri::command(async)]
pub fn term_session_write(
    state: State<'_, TerminalState>,
    session_id: String,
    data: String,
) -> Result<usize, String> {
    let conn = state
        .sessions
        .lock()
        .map_err(|_| "终端状态锁异常")?
        .get(&session_id)
        .cloned()
        .ok_or("会话不存在或已关闭")?;
    conn.lock()
        .map_err(|_| "会话状态锁异常")?
        .write(data.as_bytes())
        .map_err(|e| format!("终端写入失败：{e}"))?;
    Ok(data.len())
}
#[tauri::command(async)]
pub fn term_session_close(
    state: State<'_, TerminalState>,
    session_id: String,
) -> Result<bool, String> {
    if let Some(conn) = state
        .sessions
        .lock()
        .map_err(|_| "终端状态锁异常")?
        .remove(&session_id)
    {
        conn.lock().map_err(|_| "会话状态锁异常")?.close();
    }
    Ok(true)
}
#[tauri::command(async)]
pub fn term_session_close_all(state: State<'_, TerminalState>) -> Result<bool, String> {
    let sessions = std::mem::take(&mut *state.sessions.lock().map_err(|_| "终端状态锁异常")?);
    for (_, conn) in sessions {
        if let Ok(mut c) = conn.lock() {
            c.close();
        }
    }
    Ok(true)
}
