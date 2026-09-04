use serde::Deserialize;
use serde_json::{json, Value};
use base64::Engine;
use encoding_rs::GBK;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct ProtocolServer {
    running: Arc<AtomicBool>,
    requests: Arc<AtomicU64>,
    address: String,
    clients: Arc<Mutex<HashMap<String, TcpStream>>>,
    client_stats: Arc<Mutex<HashMap<String, TcpClientStats>>>,
}

#[derive(Clone, Default)]
struct TcpClientStats {
    connected_at_ms: u64,
    last_active_ms: u64,
    requests: u64,
    rx_bytes: u64,
    tx_bytes: u64,
    online: bool,
}

struct ModbusMemory {
    coils: Vec<bool>,
    discrete_inputs: Vec<bool>,
    holding_registers: Vec<u16>,
    input_registers: Vec<u16>,
}

pub struct ProtocolServerState {
    servers: Mutex<HashMap<String, ProtocolServer>>,
    memory: Arc<Mutex<ModbusMemory>>,
}

struct TcpClientSession {
    stream: TcpStream,
    peer: String,
    connected: Arc<AtomicBool>,
}

pub struct ProtocolClientState {
    session: Mutex<Option<TcpClientSession>>,
}

impl Default for ProtocolClientState {
    fn default() -> Self { Self { session: Mutex::new(None) } }
}

impl Default for ProtocolServerState {
    fn default() -> Self {
        Self {
            servers: Mutex::new(HashMap::new()),
            memory: Arc::new(Mutex::new(ModbusMemory {
                coils: vec![false; 65_536],
                discrete_inputs: vec![false; 65_536],
                holding_registers: vec![0; 65_536],
                input_registers: vec![0; 65_536],
            })),
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct RawExchangeRequest {
    host: String,
    port: u16,
    payload: String,
    encoding: Option<String>,
    timeout_ms: Option<u64>,
    input_mode: Option<String>,
    text_encoding: Option<String>,
    parse_escapes: Option<bool>,
    line_ending: Option<String>,
    custom_suffix_hex: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct TcpClientConnectRequest {
    host: String,
    port: u16,
    timeout_ms: Option<u64>,
    text_encoding: Option<String>,
    framing_mode: Option<String>,
    frame_delimiter_hex: Option<String>,
    fixed_frame_length: Option<usize>,
}

#[derive(Clone, Deserialize)]
pub struct TcpClientSendRequest {
    payload: String,
    encoding: Option<String>,
    timeout_ms: Option<u64>,
    input_mode: Option<String>,
    text_encoding: Option<String>,
    parse_escapes: Option<bool>,
    line_ending: Option<String>,
    custom_suffix_hex: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct ModbusRequest {
    host: String,
    port: u16,
    unit_id: u8,
    function: u8,
    address: u16,
    quantity: Option<u16>,
    values: Option<Vec<u16>>,
    timeout_ms: Option<u64>,
}

#[derive(Clone, Deserialize)]
pub struct ModbusRtuRequest {
    port_name: String,
    baud_rate: u32,
    data_bits: Option<u8>,
    stop_bits: Option<u8>,
    parity: Option<String>,
    unit_id: u8,
    function: u8,
    address: u16,
    quantity: Option<u16>,
    values: Option<Vec<u16>>,
    timeout_ms: Option<u64>,
}

#[derive(Clone, Deserialize)]
pub struct ProtocolServerRequest {
    kind: String,
    host: String,
    port: u16,
    response: Option<String>,
    encoding: Option<String>,
    serial_port: Option<String>,
    baud_rate: Option<u32>,
    parity: Option<String>,
    response_mode: Option<String>,
    response_input_mode: Option<String>,
    text_encoding: Option<String>,
    parse_escapes: Option<bool>,
    line_ending: Option<String>,
    custom_suffix_hex: Option<String>,
    framing_mode: Option<String>,
    frame_delimiter_hex: Option<String>,
    fixed_frame_length: Option<usize>,
}

#[derive(Clone, Deserialize)]
pub struct ProtocolServerSendRequest {
    kind: String,
    peer: Option<String>,
    payload: String,
    encoding: Option<String>,
    input_mode: Option<String>,
    text_encoding: Option<String>,
    parse_escapes: Option<bool>,
    line_ending: Option<String>,
    custom_suffix_hex: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct PayloadPreviewRequest {
    payload: String,
    input_mode: Option<String>,
    text_encoding: Option<String>,
    parse_escapes: Option<bool>,
    line_ending: Option<String>,
    custom_suffix_hex: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct TcpLoopbackRequest {
    payload: String,
    encoding: Option<String>,
    response: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
pub struct ModbusMemoryRequest {
    area: String,
    address: u16,
    values: Vec<u16>,
}

fn timeout(value: Option<u64>) -> Duration {
    Duration::from_millis(value.unwrap_or(2_000).clamp(100, 30_000))
}

fn epoch_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    millis.min(u64::MAX as u128) as u64
}

fn decode_payload(text: &str, encoding: Option<&str>) -> Result<Vec<u8>, String> {
    match encoding.unwrap_or("hex").to_ascii_lowercase().as_str() {
        "ascii" | "text" | "utf8" => Ok(text.as_bytes().to_vec()),
        "hex" => {
            let compact = text
                .chars()
                .filter(|ch| !ch.is_ascii_whitespace() && *ch != '-' && *ch != ':')
                .collect::<String>();
            if compact.len() % 2 != 0 {
                return Err("十六进制数据长度必须为偶数".into());
            }
            (0..compact.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&compact[index..index + 2], 16).map_err(|_| format!("无效十六进制数据：{}", &compact[index..index + 2])))
                .collect()
        }
        other => Err(format!("不支持的数据编码：{other}")),
    }
}

fn parse_hex_flexible(text: &str) -> Result<Vec<u8>, String> {
    let mut compact = String::new();
    let mut index = 0_usize;
    let chars = text.chars().collect::<Vec<_>>();
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_whitespace() || matches!(ch, '-' | ':' | ',') { index += 1; continue; }
        if ch == '0' && chars.get(index + 1).is_some_and(|next| *next == 'x' || *next == 'X') { index += 2; continue; }
        if !ch.is_ascii_hexdigit() { return Err(format!("第 {} 个字符 `{}` 不是十六进制；请删除它或切换到文本模式", index + 1, ch)); }
        compact.push(ch);
        index += 1;
    }
    if compact.len() % 2 != 0 {
        return Err(format!("第 {} 个 HEX 字符 `{}` 缺少配对半字节；请补全为两位，或切换到文本模式", compact.len(), compact.chars().last().unwrap_or('?')));
    }
    (0..compact.len()).step_by(2).map(|offset| u8::from_str_radix(&compact[offset..offset + 2], 16).map_err(|_| "HEX 解析失败".to_string())).collect()
}

fn parse_text_escapes(text: &str) -> Result<String, String> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0_usize;
    while index < chars.len() {
        if chars[index] != '\\' { output.push(chars[index]); index += 1; continue; }
        let escape_at = index + 1;
        index += 1;
        let escaped = *chars.get(index).ok_or_else(|| format!("第 {escape_at} 个字符是未完成的转义符"))?;
        match escaped {
            'r' => output.push('\r'), 'n' => output.push('\n'), 't' => output.push('\t'), '\\' => output.push('\\'),
            'x' => {
                let digits = chars.get(index + 1..index + 3).ok_or_else(|| format!("第 {escape_at} 个转义需要两位 HEX：\\xNN"))?.iter().collect::<String>();
                let value = u8::from_str_radix(&digits, 16).map_err(|_| format!("无效转义 \\x{digits}"))?;
                output.push(char::from(value)); index += 2;
            }
            'u' => {
                let digits = chars.get(index + 1..index + 5).ok_or_else(|| format!("第 {escape_at} 个转义需要四位 HEX：\\uNNNN"))?.iter().collect::<String>();
                let value = u32::from_str_radix(&digits, 16).map_err(|_| format!("无效转义 \\u{digits}"))?;
                output.push(char::from_u32(value).ok_or_else(|| format!("无效 Unicode 码点：{digits}"))?); index += 4;
            }
            other => return Err(format!("不支持的转义 \\{other}；支持 \\r、\\n、\\t、\\\\、\\xNN、\\uNNNN")),
        }
        index += 1;
    }
    Ok(output)
}

fn encode_text(text: &str, encoding: &str) -> Result<Vec<u8>, String> {
    match encoding.to_ascii_lowercase().replace('_', "-").as_str() {
        "utf-8" | "utf8" => Ok(text.as_bytes().to_vec()),
        "ascii" => {
            if let Some((index, ch)) = text.char_indices().find(|(_, ch)| !ch.is_ascii()) {
                return Err(format!("ASCII 无法表示第 {} 个字节位置的字符 `{}`；请选择 UTF-8 或 GB18030", index + 1, ch));
            }
            Ok(text.as_bytes().to_vec())
        }
        "gbk" | "gb18030" => {
            let (bytes, _, had_errors) = GBK.encode(text);
            if had_errors { Err("GB18030 无法无损编码部分字符；请选择 UTF-8".into()) } else { Ok(bytes.into_owned()) }
        }
        "utf-16le" => Ok(text.encode_utf16().flat_map(u16::to_le_bytes).collect()),
        "utf-16be" => Ok(text.encode_utf16().flat_map(u16::to_be_bytes).collect()),
        "iso-8859-1" | "latin1" => text.chars().enumerate().map(|(index, ch)| if (ch as u32) <= 255 { Ok(ch as u8) } else { Err(format!("ISO-8859-1 无法表示第 {} 个字符 `{}`", index + 1, ch)) }).collect(),
        other => Err(format!("不支持的文本编码：{other}")),
    }
}

fn decode_bytes_text(bytes: &[u8], encoding: &str) -> String {
    match encoding.to_ascii_lowercase().replace('_', "-").as_str() {
        "ascii" | "utf-8" | "utf8" => String::from_utf8_lossy(bytes).into_owned(),
        "gbk" | "gb18030" => { let (text, _, _) = GBK.decode(bytes); text.into_owned() },
        "utf-16le" => String::from_utf16_lossy(&bytes.chunks_exact(2).map(|pair|u16::from_le_bytes([pair[0],pair[1]])).collect::<Vec<_>>()),
        "utf-16be" => String::from_utf16_lossy(&bytes.chunks_exact(2).map(|pair|u16::from_be_bytes([pair[0],pair[1]])).collect::<Vec<_>>()),
        "iso-8859-1" | "latin1" => bytes.iter().map(|byte|char::from(*byte)).collect(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn decode_payload_advanced(payload: &str, input_mode: Option<&str>, text_encoding: Option<&str>, parse_escapes: bool, line_ending: Option<&str>, custom_suffix_hex: Option<&str>, legacy_encoding: Option<&str>) -> Result<Vec<u8>, String> {
    let mode = input_mode.unwrap_or_else(|| if legacy_encoding.unwrap_or("hex").eq_ignore_ascii_case("hex") { "hex" } else { "text" });
    let mut bytes = match mode.to_ascii_lowercase().as_str() {
        "hex" => parse_hex_flexible(payload)?,
        "base64" => base64::engine::general_purpose::STANDARD.decode(payload.chars().filter(|ch|!ch.is_ascii_whitespace()).collect::<String>()).map_err(|error|format!("Base64 无效：{error}"))?,
        "text" => { let text = if parse_escapes { parse_text_escapes(payload)? } else { payload.to_string() }; encode_text(&text, text_encoding.unwrap_or("utf-8"))? },
        other => return Err(format!("不支持的输入模式：{other}")),
    };
    match line_ending.unwrap_or("none").to_ascii_lowercase().as_str() {
        "none" | "" => {}, "cr" => bytes.push(0x0D), "lf" => bytes.push(0x0A), "crlf" => bytes.extend_from_slice(&[0x0D,0x0A]),
        "custom" => bytes.extend_from_slice(&parse_hex_flexible(custom_suffix_hex.unwrap_or(""))?),
        other => return Err(format!("不支持的结尾符：{other}")),
    }
    Ok(bytes)
}

#[derive(Clone, Debug)]
enum TcpFraming {
    Raw,
    Delimiter(Vec<u8>),
    Fixed(usize),
    ModbusTcp,
}

impl TcpFraming {
    fn label(&self) -> &'static str {
        match self {
            TcpFraming::Raw => "chunk",
            TcpFraming::Delimiter(_) => "delimiter",
            TcpFraming::Fixed(_) => "fixed",
            TcpFraming::ModbusTcp => "modbus_tcp",
        }
    }
}

fn tcp_framing(mode: Option<&str>, delimiter_hex: Option<&str>, fixed_length: Option<usize>, default_mode: &str) -> Result<TcpFraming, String> {
    let requested = mode.unwrap_or(default_mode).to_ascii_lowercase().replace(['-', ' '], "_");
    match requested.as_str() {
        "" | "raw" | "chunk" => Ok(TcpFraming::Raw),
        "delimiter" | "line" => {
            let delimiter = parse_hex_flexible(delimiter_hex.unwrap_or("0D 0A"))?;
            if delimiter.is_empty() { return Err("分隔符分帧需要至少 1 个 HEX 字节".into()); }
            Ok(TcpFraming::Delimiter(delimiter))
        }
        "fixed" | "fixed_length" => Ok(TcpFraming::Fixed(fixed_length.unwrap_or(1).clamp(1, 1_048_576))),
        "modbus_tcp" | "mbap" | "mbap_length" => Ok(TcpFraming::ModbusTcp),
        other => Err(format!("不支持的 TCP 分帧模式：{other}")),
    }
}

fn drain_tcp_frames(buffer: &mut Vec<u8>, incoming: &[u8], framing: &TcpFraming) -> Vec<Vec<u8>> {
    if matches!(framing, TcpFraming::Raw) {
        return if incoming.is_empty() { Vec::new() } else { vec![incoming.to_vec()] };
    }

    const MAX_BUFFERED_FRAME: usize = 1_048_576;
    buffer.extend_from_slice(incoming);
    let mut frames = Vec::new();
    match framing {
        TcpFraming::Raw => {}
        TcpFraming::Delimiter(delimiter) => {
            while let Some(offset) = buffer.windows(delimiter.len()).position(|window| window == delimiter.as_slice()) {
                frames.push(buffer.drain(..offset + delimiter.len()).collect());
            }
        }
        TcpFraming::Fixed(length) => {
            while buffer.len() >= *length {
                frames.push(buffer.drain(..*length).collect());
            }
        }
        TcpFraming::ModbusTcp => {
            loop {
                if buffer.len() < 7 { break; }
                let declared = u16::from_be_bytes([buffer[4], buffer[5]]) as usize;
                let total = 6 + declared;
                if !(8..=260).contains(&total) {
                    frames.push(std::mem::take(buffer));
                    break;
                }
                if buffer.len() < total { break; }
                frames.push(buffer.drain(..total).collect());
            }
        }
    }
    if buffer.len() > MAX_BUFFERED_FRAME {
        frames.push(std::mem::take(buffer));
    }
    frames
}

#[tauri::command]
pub fn protocol_payload_preview(req: PayloadPreviewRequest) -> Result<Value, String> {
    let bytes = decode_payload_advanced(&req.payload, req.input_mode.as_deref(), req.text_encoding.as_deref(), req.parse_escapes.unwrap_or(false), req.line_ending.as_deref(), req.custom_suffix_hex.as_deref(), None)?;
    let preview_len = bytes.len().min(256);
    Ok(json!({
        "ok":true,
        "hex":hex(&bytes[..preview_len]),
        "byte_count":bytes.len(),
        "character_count":req.payload.chars().count(),
        "preview_bytes":preview_len,
        "truncated":bytes.len() > preview_len,
        "base64":base64::engine::general_purpose::STANDARD.encode(&bytes[..preview_len]),
        "text":decode_bytes_text(&bytes[..preview_len], req.text_encoding.as_deref().unwrap_or("utf-8"))
    }))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect::<Vec<_>>().join(" ")
}

fn exchange_result(sent: &[u8], received: &[u8], elapsed: Duration, peer: String, text_encoding: &str) -> Value {
    let sequence = EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    json!({
        "ok":true, "sequence":sequence, "peer":peer, "elapsed_ms":elapsed.as_millis(),
        "timestamp_ms":epoch_ms(),
        "sent_bytes":sent.len(), "received_bytes":received.len(),
        "sent_hex":hex(sent), "received_hex":hex(received),
        "sent_base64":base64::engine::general_purpose::STANDARD.encode(sent),
        "received_base64":base64::engine::general_purpose::STANDARD.encode(received),
        "sent_text":decode_bytes_text(sent, text_encoding),
        "received_text":decode_bytes_text(received, text_encoding)
    })
}

fn raw_server_reply(configured: &[u8], received: &[u8]) -> Vec<u8> {
    if configured.is_empty() { received.to_vec() } else { configured.to_vec() }
}

fn tcp_loopback_blocking(req: TcpLoopbackRequest) -> Result<Value, String> {
    let payload = decode_payload(&req.payload, req.encoding.as_deref())?;
    if payload.is_empty() { return Err("闭环测试报文不能为空".into()); }
    let configured = decode_payload(req.response.as_deref().unwrap_or(""), req.encoding.as_deref())?;
    let expected = raw_server_reply(&configured, &payload);
    let wait = timeout(req.timeout_ms);
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| format!("创建本机测试 Server 失败：{error}"))?;
    let address = listener.local_addr().map_err(|error| format!("读取测试地址失败：{error}"))?;
    let server_expected = expected.clone();
    let server = std::thread::spawn(move || -> Result<Vec<u8>, String> {
        let (mut stream, _) = listener.accept().map_err(|error| format!("测试 Server 接收连接失败：{error}"))?;
        stream.set_read_timeout(Some(wait)).ok();
        stream.set_write_timeout(Some(wait)).ok();
        let mut received = vec![0_u8; 65_535];
        let size = stream.read(&mut received).map_err(|error| format!("测试 Server 接收失败：{error}"))?;
        received.truncate(size);
        stream.write_all(&server_expected).map_err(|error| format!("测试 Server 应答失败：{error}"))?;
        stream.flush().ok();
        Ok(received)
    });
    let started = Instant::now();
    let mut client = TcpStream::connect_timeout(&address, wait).map_err(|error| format!("测试 Client 连接失败：{error}"))?;
    client.set_read_timeout(Some(wait)).ok();
    client.set_write_timeout(Some(wait)).ok();
    client.write_all(&payload).map_err(|error| format!("测试 Client 发送失败：{error}"))?;
    client.flush().ok();
    client.shutdown(Shutdown::Write).ok();
    let mut received = Vec::new();
    client.read_to_end(&mut received).map_err(|error| format!("测试 Client 接收失败：{error}"))?;
    let server_received = server.join().map_err(|_| "测试 Server 线程异常退出".to_string())??;
    let matched = server_received == payload && received == expected;
    Ok(json!({
        "ok": matched, "matched": matched, "address": address.to_string(),
        "elapsed_ms": started.elapsed().as_millis(),
        "client_sent_hex": hex(&payload), "server_received_hex": hex(&server_received),
        "server_sent_hex": hex(&expected), "client_received_hex": hex(&received),
        "sent_bytes": payload.len(), "received_bytes": received.len(),
        "echo_mode": configured.is_empty()
    }))
}

#[tauri::command]
pub async fn protocol_tcp_loopback_test(req: TcpLoopbackRequest) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || tcp_loopback_blocking(req))
        .await
        .map_err(|error| format!("TCP 闭环任务失败：{error}"))?
}

fn resolve(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("解析目标失败：{error}"))?
        .next()
        .ok_or_else(|| "目标没有可用地址".into())
}

fn tcp_exchange_blocking(req: RawExchangeRequest) -> Result<Value, String> {
    let text_encoding = req.text_encoding.clone().unwrap_or_else(|| "utf-8".into());
    let payload = decode_payload_advanced(&req.payload, req.input_mode.as_deref(), req.text_encoding.as_deref(), req.parse_escapes.unwrap_or(false), req.line_ending.as_deref(), req.custom_suffix_hex.as_deref(), req.encoding.as_deref())?;
    let wait = timeout(req.timeout_ms);
    let peer = resolve(&req.host, req.port)?;
    let started = Instant::now();
    let mut stream = TcpStream::connect_timeout(&peer, wait)
        .map_err(|error| format!("连接 {} 失败：{error}", peer))?;
    stream.set_read_timeout(Some(wait)).ok();
    stream.set_write_timeout(Some(wait)).ok();
    stream.write_all(&payload).map_err(|error| format!("发送失败：{error}"))?;
    stream.flush().ok();
    let mut received = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(size) => {
                received.extend_from_slice(&chunk[..size]);
                if size < chunk.len() { break; }
            }
            Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => break,
            Err(error) => return Err(format!("接收失败：{error}")),
        }
    }
    Ok(exchange_result(&payload, &received, started.elapsed(), peer.to_string(), &text_encoding))
}

#[tauri::command]
pub async fn protocol_tcp_exchange(req: RawExchangeRequest) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || tcp_exchange_blocking(req))
        .await
        .map_err(|error| format!("TCP 调试任务失败：{error}"))?
}

#[tauri::command]
pub fn protocol_tcp_client_connect(app: AppHandle, state: State<'_, ProtocolClientState>, req: TcpClientConnectRequest) -> Result<Value, String> {
    let wait = timeout(req.timeout_ms);
    let peer = resolve(&req.host, req.port)?;
    let framing = tcp_framing(req.framing_mode.as_deref(), req.frame_delimiter_hex.as_deref(), req.fixed_frame_length, "raw")?;
    let started = Instant::now();
    let stream = TcpStream::connect_timeout(&peer, wait).map_err(|error| format!("连接 {peer} 失败：{error}"))?;
    stream.set_read_timeout(Some(wait)).ok();
    stream.set_write_timeout(Some(wait)).ok();
    stream.set_nodelay(true).ok();
    let peer_text = peer.to_string();
    let connected = Arc::new(AtomicBool::new(true));
    let reader_connected = connected.clone();
    let reader_peer = peer_text.clone();
    let reader_encoding = req.text_encoding.unwrap_or_else(|| "utf-8".into());
    let mut reader = stream.try_clone().map_err(|error| format!("创建接收通道失败：{error}"))?;
    reader.set_read_timeout(Some(Duration::from_millis(200))).ok();
    std::thread::spawn(move || {
        let mut received = vec![0_u8; 65_535];
        let mut frame_buffer = Vec::new();
        while reader_connected.load(Ordering::Relaxed) {
            match reader.read(&mut received) {
                Ok(0) => break,
                Ok(size) => {
                    for packet in drain_tcp_frames(&mut frame_buffer, &received[..size], &framing) {
                        let sequence=EVENT_SEQUENCE.fetch_add(1,Ordering::Relaxed);
                        let _ = app.emit("protocol-client:event", json!({"sequence":sequence,"timestamp_ms":epoch_ms(),"peer":reader_peer,"received_hex":hex(&packet),"received_base64":base64::engine::general_purpose::STANDARD.encode(&packet),"received_text":decode_bytes_text(&packet,&reader_encoding),"received_bytes":packet.len(),"kind":framing.label()}));
                    }
                }
                Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
                Err(_) => break,
            }
        }
        reader_connected.store(false, Ordering::Relaxed);
        let _ = app.emit("protocol-client:status", json!({"connected":false,"peer":reader_peer}));
    });
    let mut session = state.session.lock().map_err(|_| "TCP Client 状态锁异常")?;
    if let Some(old) = session.take() { old.stream.shutdown(Shutdown::Both).ok(); }
    *session = Some(TcpClientSession { stream, peer: peer_text.clone(), connected });
    Ok(json!({"ok":true,"connected":true,"peer":peer_text,"elapsed_ms":started.elapsed().as_millis()}))
}

#[tauri::command]
pub fn protocol_tcp_client_send(state: State<'_, ProtocolClientState>, req: TcpClientSendRequest) -> Result<Value, String> {
    let text_encoding = req.text_encoding.clone().unwrap_or_else(|| "utf-8".into());
    let payload = decode_payload_advanced(&req.payload, req.input_mode.as_deref(), req.text_encoding.as_deref(), req.parse_escapes.unwrap_or(false), req.line_ending.as_deref(), req.custom_suffix_hex.as_deref(), req.encoding.as_deref())?;
    if payload.is_empty() { return Err("发送报文不能为空".into()); }
    let wait = timeout(req.timeout_ms);
    let mut guard = state.session.lock().map_err(|_| "TCP Client 状态锁异常")?;
    let session = guard.as_mut().ok_or("TCP Client 尚未连接，请先点击连接")?;
    session.stream.set_write_timeout(Some(wait)).ok();
    if !session.connected.load(Ordering::Relaxed) { return Err("TCP 连接已断开，请重新连接".into()); }
    let started = Instant::now();
    session.stream.write_all(&payload).map_err(|error| format!("发送失败：{error}"))?;
    session.stream.flush().ok();
    Ok(exchange_result(&payload, &[], started.elapsed(), session.peer.clone(), &text_encoding))
}

#[tauri::command]
pub fn protocol_tcp_client_disconnect(state: State<'_, ProtocolClientState>) -> Result<bool, String> {
    if let Some(session) = state.session.lock().map_err(|_| "TCP Client 状态锁异常")?.take() {
        session.connected.store(false, Ordering::Relaxed);
        session.stream.shutdown(Shutdown::Both).ok();
    }
    Ok(true)
}

#[tauri::command]
pub fn protocol_tcp_client_status(state: State<'_, ProtocolClientState>) -> Result<Value, String> {
    let session = state.session.lock().map_err(|_| "TCP Client 状态锁异常")?;
    Ok(match session.as_ref() {
        Some(client) if client.connected.load(Ordering::Relaxed) => json!({"connected":true,"peer":client.peer}),
        Some(_) => json!({"connected":false,"peer":Value::Null}),
        None => json!({"connected":false,"peer":Value::Null}),
    })
}

#[tauri::command]
pub async fn protocol_udp_exchange(req: RawExchangeRequest) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || {
        let text_encoding = req.text_encoding.clone().unwrap_or_else(|| "utf-8".into());
        let payload = decode_payload_advanced(&req.payload, req.input_mode.as_deref(), req.text_encoding.as_deref(), req.parse_escapes.unwrap_or(false), req.line_ending.as_deref(), req.custom_suffix_hex.as_deref(), req.encoding.as_deref())?;
        let wait = timeout(req.timeout_ms);
        let peer = resolve(&req.host, req.port)?;
        let socket = UdpSocket::bind(("0.0.0.0", 0)).map_err(|error| format!("创建 UDP 套接字失败：{error}"))?;
        socket.set_read_timeout(Some(wait)).ok();
        let started = Instant::now();
        socket.send_to(&payload, peer).map_err(|error| format!("发送失败：{error}"))?;
        let mut received = vec![0_u8; 65_535];
        let size = match socket.recv_from(&mut received) {
            Ok((size, _)) => size,
            Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => 0,
            Err(error) => return Err(format!("接收失败：{error}")),
        };
        received.truncate(size);
        Ok(exchange_result(&payload, &received, started.elapsed(), peer.to_string(), &text_encoding))
    })
    .await
    .map_err(|error| format!("UDP 调试任务失败：{error}"))?
}

fn modbus_pdu(function: u8, address: u16, quantity: Option<u16>, values: Option<&[u16]>) -> Result<Vec<u8>, String> {
    let mut pdu = vec![function];
    pdu.extend_from_slice(&address.to_be_bytes());
    match function {
        1..=4 => pdu.extend_from_slice(&quantity.unwrap_or(1).clamp(1, 2_000).to_be_bytes()),
        5 | 6 => pdu.extend_from_slice(&values.and_then(|items| items.first()).copied().unwrap_or(0).to_be_bytes()),
        15 => {
            let bits = values.unwrap_or(&[]);
            let count = quantity.unwrap_or(bits.len() as u16).clamp(1, 1_968);
            let mut packed = vec![0_u8; (count as usize + 7) / 8];
            for (index, value) in bits.iter().take(count as usize).enumerate() {
                if *value != 0 { packed[index / 8] |= 1 << (index % 8); }
            }
            pdu.extend_from_slice(&count.to_be_bytes());
            pdu.push(packed.len() as u8);
            pdu.extend_from_slice(&packed);
        }
        16 => {
            let registers = values.unwrap_or(&[]);
            if registers.is_empty() || registers.len() > 123 { return Err("写多个寄存器需要 1-123 个数值".into()); }
            pdu.extend_from_slice(&(registers.len() as u16).to_be_bytes());
            pdu.push((registers.len() * 2) as u8);
            for value in registers { pdu.extend_from_slice(&value.to_be_bytes()); }
        }
        _ => return Err("支持功能码 01/02/03/04/05/06/0F/10".into()),
    }
    Ok(pdu)
}

#[tauri::command]
pub async fn protocol_modbus_tcp(req: ModbusRequest) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || {
        let function = req.function;
        let quantity = req.quantity.unwrap_or(1);
        let pdu = modbus_pdu(req.function, req.address, req.quantity, req.values.as_deref())?;
        let transaction = (Instant::now().elapsed().as_nanos() as u16).wrapping_add(req.address);
        let mut frame = Vec::with_capacity(pdu.len() + 7);
        frame.extend_from_slice(&transaction.to_be_bytes());
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
        frame.push(req.unit_id);
        frame.extend_from_slice(&pdu);
        let raw = RawExchangeRequest { host:req.host, port:req.port, payload:hex(&frame), encoding:Some("hex".into()), timeout_ms:req.timeout_ms, input_mode:Some("hex".into()), text_encoding:None, parse_escapes:None, line_ending:None, custom_suffix_hex:None };
        let mut result = tcp_exchange_blocking(raw)?;
        let received = decode_payload(result["received_hex"].as_str().unwrap_or(""), Some("hex"))?;
        if received.len() < 9 { return Err("Modbus TCP 响应过短或超时".into()); }
        if received[7] & 0x80 != 0 { return Err(format!("Modbus 异常响应，功能码 {:02X}，异常码 {:02X}", received[7], received[8])); }
        result["protocol"] = json!("Modbus TCP");
        result["unit_id"] = json!(received[6]);
        result["function"] = json!(received[7]);
        result["data_hex"] = json!(hex(&received[8..]));
        let response_pdu = &received[7..];
        match function {
            1 | 2 => {
                let byte_count = response_pdu.get(1).copied().unwrap_or(0) as usize;
                if response_pdu.len() != 2 + byte_count {
                    return Err(format!("Modbus 位响应 byte count 不一致：声明 {byte_count}，实际 {}", response_pdu.len().saturating_sub(2)));
                }
                let mut bits = Vec::with_capacity(quantity as usize);
                for index in 0..quantity as usize {
                    bits.push(response_pdu[2 + (index / 8)] & (1 << (index % 8)) != 0);
                }
                result["bits"] = json!(bits);
            }
            3 | 4 => {
                let byte_count = response_pdu.get(1).copied().unwrap_or(0) as usize;
                if byte_count != quantity as usize * 2 || response_pdu.len() != 2 + byte_count {
                    return Err(format!("Modbus 寄存器响应 byte count 不一致：期望 {}，接收 {byte_count}", quantity as usize * 2));
                }
                let registers = (0..quantity as usize)
                    .map(|index| u16::from_be_bytes([response_pdu[2 + index * 2], response_pdu[3 + index * 2]]))
                    .collect::<Vec<_>>();
                result["registers"] = json!(registers);
            }
            5 | 6 => {
                if response_pdu.len() != 5 || response_pdu != pdu.as_slice() {
                    return Err("Modbus 写单点响应与请求回显不一致".into());
                }
                result["write_ack"] = json!({"address":req.address,"quantity":1});
            }
            15 | 16 => {
                if response_pdu.len() != 5 {
                    return Err(format!("Modbus 写多点响应长度错误：{} 字节", response_pdu.len()));
                }
                let response_address = u16::from_be_bytes([response_pdu[1], response_pdu[2]]);
                let response_quantity = u16::from_be_bytes([response_pdu[3], response_pdu[4]]);
                if response_address != req.address || response_quantity != quantity {
                    return Err("Modbus 写多点响应地址/数量与请求不一致".into());
                }
                result["write_ack"] = json!({"address":response_address,"quantity":response_quantity});
            }
            _ => {}
        }
        Ok(result)
    }).await.map_err(|error| format!("Modbus TCP 任务失败：{error}"))?
}

fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc = 0xFFFF_u16;
    for byte in data {
        crc ^= *byte as u16;
        for _ in 0..8 { crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 }; }
    }
    crc
}

#[tauri::command]
pub async fn protocol_modbus_rtu(req: ModbusRtuRequest) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || {
        let mut frame = vec![req.unit_id];
        frame.extend_from_slice(&modbus_pdu(req.function, req.address, req.quantity, req.values.as_deref())?);
        frame.extend_from_slice(&crc16_modbus(&frame).to_le_bytes());
        let wait = timeout(req.timeout_ms);
        let mut builder = serialport::new(&req.port_name, req.baud_rate).timeout(wait);
        builder = builder
            .data_bits(match req.data_bits.unwrap_or(8) { 5=>serialport::DataBits::Five, 6=>serialport::DataBits::Six, 7=>serialport::DataBits::Seven, _=>serialport::DataBits::Eight })
            .stop_bits(if req.stop_bits == Some(2) { serialport::StopBits::Two } else { serialport::StopBits::One })
            .parity(match req.parity.as_deref().unwrap_or("none").to_ascii_lowercase().as_str() { "odd"=>serialport::Parity::Odd, "even"=>serialport::Parity::Even, _=>serialport::Parity::None });
        let mut port = builder.open().map_err(|error| format!("打开串口 {} 失败：{error}", req.port_name))?;
        let started = Instant::now();
        port.write_all(&frame).map_err(|error| format!("串口发送失败：{error}"))?;
        let mut received = vec![0_u8; 512];
        let size = match port.read(&mut received) { Ok(size)=>size, Err(error) if error.kind()==std::io::ErrorKind::TimedOut=>0, Err(error)=>return Err(format!("串口接收失败：{error}")) };
        received.truncate(size);
        if received.len() >= 2 {
            let body_len = received.len() - 2;
            let actual = u16::from_le_bytes([received[body_len], received[body_len + 1]]);
            if crc16_modbus(&received[..body_len]) != actual { return Err("Modbus RTU 响应 CRC 校验失败".into()); }
        }
        let mut result = exchange_result(&frame, &received, started.elapsed(), req.port_name, "utf-8");
        result["protocol"] = json!("Modbus RTU");
        Ok(result)
    }).await.map_err(|error| format!("Modbus RTU 任务失败：{error}"))?
}

fn modbus_exception(frame: &[u8], function: u8, code: u8) -> Vec<u8> {
    let mut response = frame.get(..4).unwrap_or(&[0, 0, 0, 0]).to_vec();
    response.extend_from_slice(&[0, 3]);
    response.push(*frame.get(6).unwrap_or(&1));
    response.extend_from_slice(&[function | 0x80, code]);
    response
}

fn modbus_server_response(frame: &[u8], memory: &Arc<Mutex<ModbusMemory>>) -> Result<Vec<u8>, String> {
    if frame.len() < 8 { return Err("Modbus 请求过短".into()); }
    let function = frame[7];
    let address = frame.get(8..10).map(|v| u16::from_be_bytes([v[0], v[1]]) as usize).unwrap_or(0);
    let quantity = frame.get(10..12).map(|v| u16::from_be_bytes([v[0], v[1]]) as usize).unwrap_or(0);
    let mut data = memory.lock().map_err(|_| "Modbus 内存锁异常")?;
    let mut pdu = vec![function];
    let invalid_range = |start: usize, count: usize| count == 0 || start.checked_add(count).is_none_or(|end| end > 65_536);
    match function {
        1 | 2 => {
            if quantity > 2_000 || invalid_range(address, quantity) { return Ok(modbus_exception(frame, function, 2)); }
            let bits = if function == 1 { &data.coils } else { &data.discrete_inputs };
            let mut packed = vec![0_u8; (quantity + 7) / 8];
            for index in 0..quantity { if bits[address + index] { packed[index / 8] |= 1 << (index % 8); } }
            pdu.push(packed.len() as u8); pdu.extend_from_slice(&packed);
        }
        3 | 4 => {
            if quantity > 125 || invalid_range(address, quantity) { return Ok(modbus_exception(frame, function, 2)); }
            let registers = if function == 3 { &data.holding_registers } else { &data.input_registers };
            pdu.push((quantity * 2) as u8);
            for value in &registers[address..address + quantity] { pdu.extend_from_slice(&value.to_be_bytes()); }
        }
        5 => {
            if frame.len() < 12 || address >= data.coils.len() { return Ok(modbus_exception(frame, function, 2)); }
            let raw = u16::from_be_bytes([frame[10], frame[11]]);
            if raw != 0 && raw != 0xFF00 { return Ok(modbus_exception(frame, function, 3)); }
            data.coils[address] = raw == 0xFF00;
            pdu.extend_from_slice(&frame[8..12]);
        }
        6 => {
            if frame.len() < 12 || address >= data.holding_registers.len() { return Ok(modbus_exception(frame, function, 2)); }
            data.holding_registers[address] = u16::from_be_bytes([frame[10], frame[11]]);
            pdu.extend_from_slice(&frame[8..12]);
        }
        15 => {
            let byte_count = *frame.get(12).unwrap_or(&0) as usize;
            if quantity > 1_968 || invalid_range(address, quantity) || frame.len() < 13 + byte_count { return Ok(modbus_exception(frame, function, 2)); }
            for index in 0..quantity { data.coils[address + index] = frame[13 + index / 8] & (1 << (index % 8)) != 0; }
            pdu.extend_from_slice(&frame[8..12]);
        }
        16 => {
            let byte_count = *frame.get(12).unwrap_or(&0) as usize;
            if quantity > 123 || byte_count != quantity * 2 || invalid_range(address, quantity) || frame.len() < 13 + byte_count { return Ok(modbus_exception(frame, function, 3)); }
            for index in 0..quantity { data.holding_registers[address + index] = u16::from_be_bytes([frame[13 + index * 2], frame[14 + index * 2]]); }
            pdu.extend_from_slice(&frame[8..12]);
        }
        _ => return Ok(modbus_exception(frame, function, 1)),
    }
    let mut response = frame[..4].to_vec();
    response.extend_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
    response.push(frame[6]);
    response.extend_from_slice(&pdu);
    Ok(response)
}

fn emit_server_event(app: &AppHandle, kind: &str, peer: SocketAddr, received: &[u8], sent: &[u8], text_encoding: &str, frame_kind: &str) {
    let sequence = EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let _ = app.emit("protocol-server:event", json!({
        "sequence":sequence,"timestamp_ms":epoch_ms(),"kind":kind,"peer":peer.to_string(),"received_hex":hex(received),"sent_hex":hex(sent),
        "received_base64":base64::engine::general_purpose::STANDARD.encode(received),"sent_base64":base64::engine::general_purpose::STANDARD.encode(sent),
        "received_text":decode_bytes_text(received,text_encoding),"sent_text":decode_bytes_text(sent,text_encoding),
        "received_bytes":received.len(),"sent_bytes":sent.len(),"frame_kind":frame_kind
    }));
}

fn insert_server(state: &ProtocolServerState, kind: &str, address: String, running: Arc<AtomicBool>, requests: Arc<AtomicU64>, clients: Arc<Mutex<HashMap<String, TcpStream>>>, client_stats: Arc<Mutex<HashMap<String, TcpClientStats>>>) -> Result<(), String> {
    let mut servers = state.servers.lock().map_err(|_| "协议服务状态锁异常")?;
    if servers.get(kind).is_some_and(|server| server.running.load(Ordering::Relaxed)) {
        return Err(format!("{kind} 服务已在运行，请先停止"));
    }
    servers.insert(kind.into(), ProtocolServer { running, requests, address, clients, client_stats });
    Ok(())
}

#[tauri::command]
pub fn protocol_server_start(app: AppHandle, state: State<'_, ProtocolServerState>, req: ProtocolServerRequest) -> Result<bool, String> {
    let kind = req.kind.to_ascii_lowercase();
    if !matches!(kind.as_str(), "tcp" | "udp" | "modbus_tcp" | "modbus_udp" | "modbus_rtu") { return Err("未知服务类型".into()); }
    let running = Arc::new(AtomicBool::new(true));
    let requests = Arc::new(AtomicU64::new(0));
    let clients = Arc::new(Mutex::new(HashMap::new()));
    let client_stats = Arc::new(Mutex::new(HashMap::new()));
    let address = if kind == "modbus_rtu" { req.serial_port.clone().unwrap_or_default() } else { format!("{}:{}", req.host, req.port) };
    let reply = decode_payload_advanced(req.response.as_deref().unwrap_or(""), req.response_input_mode.as_deref(), req.text_encoding.as_deref(), req.parse_escapes.unwrap_or(false), req.line_ending.as_deref(), req.custom_suffix_hex.as_deref(), req.encoding.as_deref())?;
    let response_mode = req.response_mode.unwrap_or_else(|| if reply.is_empty() { "echo".into() } else { "fixed".into() });
    let text_encoding = req.text_encoding.unwrap_or_else(|| "utf-8".into());
    if kind == "modbus_rtu" {
        let port_name = req.serial_port.filter(|value| !value.trim().is_empty()).ok_or("请选择 Modbus RTU Server 串口")?;
        let wait = Duration::from_millis(200);
        let mut builder = serialport::new(&port_name, req.baud_rate.unwrap_or(9_600)).timeout(wait);
        builder = builder.parity(match req.parity.as_deref().unwrap_or("none").to_ascii_lowercase().as_str() { "odd"=>serialport::Parity::Odd, "even"=>serialport::Parity::Even, _=>serialport::Parity::None });
        let mut serial = builder.open().map_err(|error| format!("打开 RTU Server 串口 {port_name} 失败：{error}"))?;
        insert_server(&state, &kind, port_name.clone(), running.clone(), requests.clone(), clients.clone(), client_stats.clone())?;
        let memory = state.memory.clone();
        let thread_kind = kind.clone();
        let thread_running = running.clone();
        let thread_text_encoding = text_encoding.clone();
        std::thread::spawn(move || {
            let _ = app.emit("protocol-server:status", json!({"kind":thread_kind,"running":true,"address":port_name}));
            let mut received = vec![0_u8; 256];
            while thread_running.load(Ordering::Relaxed) {
                let size = match serial.read(&mut received) { Ok(size)=>size, Err(error) if error.kind()==std::io::ErrorKind::TimedOut=>continue, Err(_)=>break };
                if size < 4 { continue; }
                let packet = &received[..size];
                let body_len = size - 2;
                let actual_crc = u16::from_le_bytes([packet[body_len], packet[body_len + 1]]);
                if crc16_modbus(&packet[..body_len]) != actual_crc { continue; }
                requests.fetch_add(1, Ordering::Relaxed);
                let unit = packet[0]; let pdu = &packet[1..body_len];
                let mut tcp_frame = vec![0, 1, 0, 0];
                tcp_frame.extend_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
                tcp_frame.push(unit); tcp_frame.extend_from_slice(pdu);
                let tcp_response = modbus_server_response(&tcp_frame, &memory).unwrap_or_default();
                let mut sent = if tcp_response.len() >= 8 { tcp_response[6..].to_vec() } else { Vec::new() };
                if !sent.is_empty() { sent.extend_from_slice(&crc16_modbus(&sent).to_le_bytes()); let _ = serial.write_all(&sent); }
                emit_server_event(&app, &thread_kind, SocketAddr::from(([127,0,0,1],0)), packet, &sent, &thread_text_encoding, "rtu");
            }
            thread_running.store(false, Ordering::Relaxed);
            let _ = app.emit("protocol-server:status", json!({"kind":thread_kind,"running":false}));
        });
    } else if kind.ends_with("tcp") || kind == "tcp" {
        let framing = tcp_framing(req.framing_mode.as_deref(), req.frame_delimiter_hex.as_deref(), req.fixed_frame_length, if kind == "modbus_tcp" { "modbus_tcp" } else { "raw" })?;
        let listener = TcpListener::bind((&*req.host, req.port)).map_err(|error| format!("绑定 {address} 失败：{error}"))?;
        listener.set_nonblocking(true).map_err(|error| error.to_string())?;
        insert_server(&state, &kind, address.clone(), running.clone(), requests.clone(), clients.clone(), client_stats.clone())?;
        let memory = state.memory.clone();
        let thread_kind = kind.clone();
        let thread_running = running.clone();
        let thread_clients = clients.clone();
        let thread_client_stats = client_stats.clone();
        let thread_response_mode = response_mode.clone();
        let thread_text_encoding = text_encoding.clone();
        let thread_framing = framing.clone();
        std::thread::spawn(move || {
            let _ = app.emit("protocol-server:status", json!({"kind":thread_kind,"running":true,"address":address}));
            while thread_running.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, peer)) => {
                        let peer_key = peer.to_string();
                        if let Ok(writer) = stream.try_clone() {
                            if let Ok(mut connected) = thread_clients.lock() { connected.insert(peer_key.clone(), writer); }
                        }
                        if let Ok(mut stats) = thread_client_stats.lock() {
                            let now = epoch_ms();
                            stats.insert(peer_key.clone(), TcpClientStats { connected_at_ms: now, last_active_ms: now, online: true, ..Default::default() });
                        }
                        let client_running = thread_running.clone();
                        let client_requests = requests.clone();
                        let client_memory = memory.clone();
                        let client_reply = reply.clone();
                        let client_kind = thread_kind.clone();
                        let client_app = app.clone();
                        let client_clients = thread_clients.clone();
                        let client_stats = thread_client_stats.clone();
                        let client_response_mode = thread_response_mode.clone();
                        let client_text_encoding = thread_text_encoding.clone();
                        let client_framing = thread_framing.clone();
                        std::thread::spawn(move || {
                            let wait = Duration::from_millis(200);
                            stream.set_read_timeout(Some(wait)).ok(); stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
                            stream.set_nodelay(true).ok();
                            let mut received = vec![0_u8; 65_535];
                            let mut frame_buffer = Vec::new();
                            while client_running.load(Ordering::Relaxed) {
                                let size = match stream.read(&mut received) {
                                    Ok(0) => break,
                                    Ok(size) => size,
                                    Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
                                    Err(_) => break,
                                };
                                let mut write_failed = false;
                                for packet in drain_tcp_frames(&mut frame_buffer, &received[..size], &client_framing) {
                                    client_requests.fetch_add(1, Ordering::Relaxed);
                                    let sent = if client_kind == "modbus_tcp" { modbus_server_response(&packet, &client_memory).unwrap_or_default() } else { match client_response_mode.as_str() { "none"=>Vec::new(), "fixed"=>client_reply.clone(), _=>packet.clone() } };
                                    if !sent.is_empty() && stream.write_all(&sent).is_err() { write_failed = true; break; }
                                    if let Ok(mut stats) = client_stats.lock() {
                                        if let Some(item) = stats.get_mut(&peer_key) {
                                            item.requests = item.requests.saturating_add(1);
                                            item.rx_bytes = item.rx_bytes.saturating_add(packet.len() as u64);
                                            item.tx_bytes = item.tx_bytes.saturating_add(sent.len() as u64);
                                            item.last_active_ms = epoch_ms();
                                            item.online = true;
                                        }
                                    }
                                    emit_server_event(&client_app, &client_kind, peer, &packet, &sent, &client_text_encoding, client_framing.label());
                                }
                                if write_failed { break; }
                            }
                            if let Ok(mut connected) = client_clients.lock() { connected.remove(&peer_key); }
                            if let Ok(mut stats) = client_stats.lock() {
                                if let Some(item) = stats.get_mut(&peer_key) {
                                    item.online = false;
                                    item.last_active_ms = epoch_ms();
                                }
                            }
                            let _ = client_app.emit("protocol-server:client", json!({"kind":client_kind,"peer":peer_key,"connected":false}));
                        });
                        let _ = app.emit("protocol-server:client", json!({"kind":thread_kind,"peer":peer.to_string(),"connected":true}));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(30)),
                    Err(_) => break,
                }
            }
            thread_running.store(false, Ordering::Relaxed);
            let _ = app.emit("protocol-server:status", json!({"kind":thread_kind,"running":false}));
        });
    } else {
        let socket = UdpSocket::bind((&*req.host, req.port)).map_err(|error| format!("绑定 {address} 失败：{error}"))?;
        socket.set_read_timeout(Some(Duration::from_millis(200))).ok();
        insert_server(&state, &kind, address.clone(), running.clone(), requests.clone(), clients.clone(), client_stats.clone())?;
        let memory = state.memory.clone();
        let thread_kind = kind.clone();
        let thread_running = running.clone();
        let thread_text_encoding = text_encoding.clone();
        let thread_response_mode = response_mode.clone();
        std::thread::spawn(move || {
            let _ = app.emit("protocol-server:status", json!({"kind":thread_kind,"running":true,"address":address}));
            let mut received = vec![0_u8; 65_535];
            while thread_running.load(Ordering::Relaxed) {
                match socket.recv_from(&mut received) {
                    Ok((size, peer)) => {
                        requests.fetch_add(1, Ordering::Relaxed);
                        let packet = &received[..size];
                        let sent = if thread_kind == "modbus_udp" { modbus_server_response(packet, &memory).unwrap_or_default() } else { match thread_response_mode.as_str() { "none"=>Vec::new(), "fixed"=>reply.clone(), _=>packet.to_vec() } };
                        if !sent.is_empty() { let _ = socket.send_to(&sent, peer); }
                        emit_server_event(&app, &thread_kind, peer, packet, &sent, &thread_text_encoding, "datagram");
                    }
                    Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                    Err(_) => break,
                }
            }
            thread_running.store(false, Ordering::Relaxed);
            let _ = app.emit("protocol-server:status", json!({"kind":thread_kind,"running":false}));
        });
    }
    Ok(true)
}

#[tauri::command]
pub fn protocol_server_stop(state: State<'_, ProtocolServerState>, kind: String) -> Result<bool, String> {
    if let Some(server) = state.servers.lock().map_err(|_| "协议服务状态锁异常")?.remove(&kind) {
        server.running.store(false, Ordering::Relaxed);
        if let Ok(clients) = server.clients.lock() { for stream in clients.values() { stream.shutdown(Shutdown::Both).ok(); } }
    }
    Ok(true)
}

#[tauri::command]
pub fn protocol_server_stop_all(state: State<'_, ProtocolServerState>) -> Result<bool, String> {
    let mut servers = state.servers.lock().map_err(|_| "协议服务状态锁异常")?;
    for server in servers.values() {
        server.running.store(false, Ordering::Relaxed);
        if let Ok(clients) = server.clients.lock() { for stream in clients.values() { stream.shutdown(Shutdown::Both).ok(); } }
    }
    servers.clear();
    Ok(true)
}

#[tauri::command]
pub fn protocol_server_status(state: State<'_, ProtocolServerState>) -> Result<Value, String> {
    let servers = state.servers.lock().map_err(|_| "协议服务状态锁异常")?;
    Ok(json!({"servers":servers.iter().map(|(kind,server)|json!({
        "kind":kind,"running":server.running.load(Ordering::Relaxed),"address":server.address,
        "requests":server.requests.load(Ordering::Relaxed),
        "clients":server.client_stats.lock().map(|items|{
            let mut clients = items.iter().map(|(peer,stats)|json!({
                "peer":peer,
                "connected_at_ms":stats.connected_at_ms,
                "last_active_ms":stats.last_active_ms,
                "requests":stats.requests,
                "rx_bytes":stats.rx_bytes,
                "tx_bytes":stats.tx_bytes,
                "online":stats.online
            })).collect::<Vec<_>>();
            clients.sort_by(|left,right| right["last_active_ms"].as_u64().cmp(&left["last_active_ms"].as_u64()));
            clients
        }).unwrap_or_default()
    })).collect::<Vec<_>>() }))
}

#[tauri::command]
pub fn protocol_server_send(state: State<'_, ProtocolServerState>, req: ProtocolServerSendRequest) -> Result<Value, String> {
    let text_encoding = req.text_encoding.clone().unwrap_or_else(|| "utf-8".into());
    let payload = decode_payload_advanced(&req.payload, req.input_mode.as_deref(), req.text_encoding.as_deref(), req.parse_escapes.unwrap_or(false), req.line_ending.as_deref(), req.custom_suffix_hex.as_deref(), req.encoding.as_deref())?;
    if payload.is_empty() { return Err("Server 发送报文不能为空".into()); }
    let (clients, client_stats) = {
        let servers = state.servers.lock().map_err(|_| "协议服务状态锁异常")?;
        let server = servers.get(&req.kind).ok_or("Server 未启动")?;
        (server.clients.clone(), server.client_stats.clone())
    };
    let mut connected = clients.lock().map_err(|_| "Server 客户端列表锁异常")?;
    let targets = connected.keys().filter(|peer| req.peer.as_deref().is_none_or(|selected| selected.is_empty() || selected == "*" || selected == peer.as_str())).cloned().collect::<Vec<_>>();
    if targets.is_empty() { return Err("没有可发送的 TCP 客户端".into()) }
    let mut sent_to = Vec::new();
    let mut failed = Vec::new();
    for peer in targets {
        match connected.get_mut(&peer).and_then(|stream| stream.write_all(&payload).ok()) {
            Some(()) => sent_to.push(peer),
            None => failed.push(peer),
        }
    }
    for peer in &failed { connected.remove(peer); }
    if let Ok(mut stats) = client_stats.lock() {
        for peer in &sent_to {
            if let Some(item) = stats.get_mut(peer) {
                item.tx_bytes = item.tx_bytes.saturating_add(payload.len() as u64);
                item.last_active_ms = epoch_ms();
            }
        }
        for peer in &failed {
            if let Some(item) = stats.get_mut(peer) {
                item.online = false;
                item.last_active_ms = epoch_ms();
            }
        }
    }
    if sent_to.is_empty() { return Err("发送失败，客户端连接已失效".into()); }
    let sequence = EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(json!({
        "ok":true,
        "sequence":sequence,
        "timestamp_ms":epoch_ms(),
        "sent_hex":hex(&payload),
        "sent_base64":base64::engine::general_purpose::STANDARD.encode(&payload),
        "sent_text":decode_bytes_text(&payload, &text_encoding),
        "sent_bytes":payload.len(),
        "sent_to":sent_to,
        "failed":failed
    }))
}

#[tauri::command]
pub fn protocol_server_set_memory(state: State<'_, ProtocolServerState>, req: ModbusMemoryRequest) -> Result<Value, String> {
    let mut memory = state.memory.lock().map_err(|_| "Modbus 内存锁异常")?;
    let start = req.address as usize;
    if start + req.values.len() > 65_536 { return Err("写入范围超过 65535".into()); }
    match req.area.to_ascii_lowercase().as_str() {
        "coil" | "coils" => for (offset, value) in req.values.iter().enumerate() { memory.coils[start + offset] = *value != 0; },
        "discrete" | "discrete_input" | "discrete_inputs" => for (offset, value) in req.values.iter().enumerate() { memory.discrete_inputs[start + offset] = *value != 0; },
        "input" | "input_register" | "input_registers" => for (offset, value) in req.values.iter().enumerate() { memory.input_registers[start + offset] = *value; },
        "holding" | "register" | "registers" | "holding_register" | "holding_registers" => for (offset, value) in req.values.iter().enumerate() { memory.holding_registers[start + offset] = *value; },
        _ => return Err("未知 Modbus 数据区，支持 holding/input/coil/discrete".into()),
    }
    Ok(json!({"ok":true,"area":req.area,"address":req.address,"count":req.values.len()}))
}

#[tauri::command]
pub fn protocol_server_get_memory(state: State<'_, ProtocolServerState>, area: String, address: u16, quantity: u16) -> Result<Value, String> {
    let memory = state.memory.lock().map_err(|_| "Modbus 内存锁异常")?;
    let start = address as usize; let count = quantity.clamp(1, 256) as usize;
    if start + count > 65_536 { return Err("读取范围超过 65535".into()); }
    let values = match area.to_ascii_lowercase().as_str() {
        "coil" | "coils" => memory.coils[start..start+count].iter().map(|value| u16::from(*value)).collect::<Vec<_>>(),
        "discrete" | "discrete_input" | "discrete_inputs" => memory.discrete_inputs[start..start+count].iter().map(|value| u16::from(*value)).collect::<Vec<_>>(),
        "input" | "input_register" | "input_registers" => memory.input_registers[start..start+count].to_vec(),
        "holding" | "register" | "registers" | "holding_register" | "holding_registers" => memory.holding_registers[start..start+count].to_vec(),
        _ => return Err("未知 Modbus 数据区，支持 holding/input/coil/discrete".into()),
    };
    Ok(json!({"area":area,"address":address,"values":values}))
}

#[tauri::command]
pub fn protocol_workbench_catalog() -> Value {
    json!({"groups":[
        {"name":"通用通信","items":["TCP Client","UDP Client","Serial Debug","Serial↔TCP","TCP Relay"]},
        {"name":"Modbus","items":["Modbus TCP","Modbus UDP","Modbus RTU","Modbus ASCII","RTU over TCP"]},
        {"name":"PLC","items":["Siemens S7 200/300/400/1200/1500","Mitsubishi MC 1E/3E/4E","Omron FINS/CIP/HostLink","Allen-Bradley EtherNet/IP"]},
        {"name":"电力/仪表","items":["IEC 60870-5-104","DL/T 645-1997/2007","DL/T 698","CJ/T 188"]},
        {"name":"应用协议","items":["HTTP/WebApi","WebSocket","MQTT","Redis","FTP"]}
    ],"implemented":["TCP Client","UDP Client","Serial Debug","Modbus TCP Client","Modbus RTU Client","TCP Server","UDP Server","Modbus TCP Server","Modbus UDP Server"]})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_hex_and_builds_modbus_frames() {
        assert_eq!(decode_payload("01 03-00:00", Some("hex")).unwrap(), [1,3,0,0]);
        assert_eq!(modbus_pdu(3, 10, Some(2), None).unwrap(), [3,0,10,0,2]);
        assert_eq!(crc16_modbus(&[1,3,0,0,0,10]), 0xCDC5);
    }

    #[test]
    fn payload_editor_encodes_utf8_chinese_and_validates_modes() {
        let utf8 = decode_payload_advanced("中文ABC", Some("text"), Some("utf-8"), false, Some("crlf"), None, None).unwrap();
        assert_eq!(hex(&utf8), "E4 B8 AD E6 96 87 41 42 43 0D 0A");
        let gbk = decode_payload_advanced("中文", Some("text"), Some("gb18030"), false, None, None, None).unwrap();
        assert_eq!(hex(&gbk), "D6 D0 CE C4");
        assert!(decode_payload_advanced("中文", Some("text"), Some("ascii"), false, None, None, None).unwrap_err().contains("UTF-8"));
        assert!(decode_payload_advanced("aaa", Some("hex"), None, false, None, None, None).unwrap_err().contains("缺少配对半字节"));
        assert_eq!(decode_payload_advanced("A\\r\\n", Some("text"), Some("utf-8"), true, None, None, None).unwrap(), b"A\r\n");
    }

    #[test]
    fn tcp_framing_handles_delimiter_fixed_and_modbus_tcp() {
        let delimiter = tcp_framing(Some("delimiter"), Some("0D 0A"), None, "raw").unwrap();
        let mut buffer = Vec::new();
        assert!(drain_tcp_frames(&mut buffer, b"abc", &delimiter).is_empty());
        assert_eq!(drain_tcp_frames(&mut buffer, b"\r\ndef\r\n", &delimiter), vec![b"abc\r\n".to_vec(), b"def\r\n".to_vec()]);

        let fixed = tcp_framing(Some("fixed"), None, Some(3), "raw").unwrap();
        let mut buffer = Vec::new();
        assert_eq!(drain_tcp_frames(&mut buffer, b"abcdefg", &fixed), vec![b"abc".to_vec(), b"def".to_vec()]);
        assert_eq!(buffer, b"g");

        let modbus = tcp_framing(Some("modbus_tcp"), None, None, "raw").unwrap();
        let mut buffer = Vec::new();
        let frame = [0, 1, 0, 0, 0, 6, 1, 3, 0, 0, 0, 2];
        assert!(drain_tcp_frames(&mut buffer, &frame[..5], &modbus).is_empty());
        assert_eq!(drain_tcp_frames(&mut buffer, &frame[5..], &modbus), vec![frame.to_vec()]);
    }

    #[test]
    fn raw_server_defaults_to_echo_and_tcp_loopback_is_real() {
        assert_eq!(raw_server_reply(&[], &[1, 2, 3]), [1, 2, 3]);
        assert_eq!(raw_server_reply(&[9], &[1, 2, 3]), [9]);
        let result = tcp_loopback_blocking(TcpLoopbackRequest {
            payload: "01 03 00 00 00 02".into(), encoding: Some("hex".into()),
            response: None, timeout_ms: Some(1_000),
        }).unwrap();
        assert_eq!(result["matched"], true);
        assert_eq!(result["client_received_hex"], "01 03 00 00 00 02");
    }

    #[test]
    fn modbus_server_reads_and_writes_holding_registers() {
        let memory = Arc::new(Mutex::new(ModbusMemory {
            coils: vec![false; 65_536],
            discrete_inputs: vec![false; 65_536],
            holding_registers: vec![0; 65_536],
            input_registers: vec![0; 65_536],
        }));
        let write = [0, 1, 0, 0, 0, 6, 1, 6, 0, 10, 0x12, 0x34];
        let write_reply = modbus_server_response(&write, &memory).unwrap();
        assert_eq!(&write_reply[7..], &[6, 0, 10, 0x12, 0x34]);
        let read = [0, 2, 0, 0, 0, 6, 1, 3, 0, 10, 0, 1];
        let read_reply = modbus_server_response(&read, &memory).unwrap();
        assert_eq!(&read_reply[7..], &[3, 2, 0x12, 0x34]);
    }

    #[test]
    fn modbus_server_keeps_four_data_areas_separate() {
        let memory = Arc::new(Mutex::new(ModbusMemory {
            coils: vec![false; 65_536],
            discrete_inputs: vec![false; 65_536],
            holding_registers: vec![0; 65_536],
            input_registers: vec![0; 65_536],
        }));
        {
            let mut data = memory.lock().unwrap();
            data.coils[2] = true;
            data.discrete_inputs[2] = false;
            data.holding_registers[4] = 0x1111;
            data.input_registers[4] = 0x2222;
        }

        let read_coil = [0, 1, 0, 0, 0, 6, 1, 1, 0, 2, 0, 1];
        let read_discrete = [0, 1, 0, 0, 0, 6, 1, 2, 0, 2, 0, 1];
        let read_holding = [0, 1, 0, 0, 0, 6, 1, 3, 0, 4, 0, 1];
        let read_input = [0, 1, 0, 0, 0, 6, 1, 4, 0, 4, 0, 1];

        assert_eq!(&modbus_server_response(&read_coil, &memory).unwrap()[7..], &[1, 1, 1]);
        assert_eq!(&modbus_server_response(&read_discrete, &memory).unwrap()[7..], &[2, 1, 0]);
        assert_eq!(&modbus_server_response(&read_holding, &memory).unwrap()[7..], &[3, 2, 0x11, 0x11]);
        assert_eq!(&modbus_server_response(&read_input, &memory).unwrap()[7..], &[4, 2, 0x22, 0x22]);
    }
}
