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
static MODBUS_TRANSACTION: AtomicU64 = AtomicU64::new(1);

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
    unit_id: Option<u8>,
    data_bits: Option<u8>,
    stop_bits: Option<u8>,
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
                    buffer.clear();
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
    let values = values.unwrap_or(&[]);
    let count = match function {
        1..=4 => quantity.unwrap_or(1) as usize,
        5 | 6 => {
            if values.len() != 1 || quantity.is_some_and(|count| count != 1) {
                return Err("Single write requires exactly one value".into());
            }
            1
        }
        15 | 16 => {
            if quantity.is_some_and(|count| count as usize != values.len()) {
                return Err("Write quantity does not match value count".into());
            }
            values.len()
        }
        _ => return Err("Supported functions: 01/02/03/04/05/06/0F/10".into()),
    };
    let limit = match function { 1 | 2 => 2000, 3 | 4 => 125, 15 => 1968, 16 => 123, _ => 1 };
    if count == 0 || count > limit { return Err(format!("Quantity must be between 1 and {limit}")); }
    if address as usize + count > 65_536 { return Err("Address range exceeds 65535".into()); }
    let mut pdu = vec![function];
    pdu.extend_from_slice(&address.to_be_bytes());
    match function {
        1..=4 => pdu.extend_from_slice(&(count as u16).to_be_bytes()),
        5 => {
            let wire: u16 = match values[0] { 0 => 0, 1 | 0xFF00 => 0xFF00, _ => return Err("Invalid coil value".into()) };
            pdu.extend_from_slice(&wire.to_be_bytes());
        }
        6 => pdu.extend_from_slice(&values[0].to_be_bytes()),
        15 => {
            if values.iter().any(|value| *value > 1) { return Err("Multiple coils require 0 or 1".into()); }
            let mut packed = vec![0_u8; count.div_ceil(8)];
            for (index, value) in values.iter().enumerate() {
                if *value != 0 { packed[index / 8] |= 1 << (index % 8); }
            }
            pdu.extend_from_slice(&(count as u16).to_be_bytes());
            pdu.push(packed.len() as u8);
            pdu.extend_from_slice(&packed);
        }
        16 => {
            pdu.extend_from_slice(&(count as u16).to_be_bytes());
            pdu.push((count * 2) as u8);
            for value in values { pdu.extend_from_slice(&value.to_be_bytes()); }
        }
        _ => return Err("支持功能码 01/02/03/04/05/06/0F/10".into()),
    }
    Ok(pdu)
}

fn remaining(deadline: Instant) -> Result<Duration, String> {
    deadline.checked_duration_since(Instant::now()).filter(|wait| !wait.is_zero())
        .ok_or_else(|| "Modbus response deadline exceeded".into())
}

// Read only the current ADU, with one deadline across all fragmented reads.
fn read_modbus_frame(mut read: impl FnMut(&mut [u8], Duration) -> std::io::Result<usize>, deadline: Instant, rtu: bool) -> Result<Vec<u8>, String> {
    let mut frame = Vec::new();
    let mut expected = if rtu { 2 } else { 6 };
    loop {
        if frame.len() == expected {
            if !rtu && expected == 6 {
                let length = u16::from_be_bytes([frame[4], frame[5]]) as usize;
                if frame[2..4] != [0, 0] || !(2..=254).contains(&length) {
                    return Err(format!("Invalid MBAP header; RX={}", hex(&frame)));
                }
                expected = 6 + length;
            } else if rtu && expected == 2 {
                expected = match frame[1] { 1..=4 => 3, 5 | 6 | 15 | 16 => 8, fc if fc & 0x80 != 0 => 5, _ => return Err(format!("Invalid RTU function; RX={}", hex(&frame))) };
            } else if rtu && expected == 3 {
                expected = frame[2] as usize + 5;
                if expected > 256 { return Err(format!("RTU frame too long; RX={}", hex(&frame))); }
            } else {
                return Ok(frame);
            }
        }
        let mut chunk = [0_u8; 260];
        let wait = remaining(deadline).map_err(|error| format!("{error}; RX={}", hex(&frame)))?;
        match read(&mut chunk[..expected - frame.len()], wait) {
            Ok(0) => return Err(format!("Incomplete Modbus response; RX={}", hex(&frame))),
            Ok(size) => frame.extend_from_slice(&chunk[..size]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("Modbus receive failed: {error}; RX={}", hex(&frame))),
        }
    }
}

fn parse_modbus_response(request: &[u8], response: &[u8]) -> Result<Value, String> {
    let function = request[0];
    let response_function = *response.first().ok_or("Empty Modbus PDU")?;
    if response_function == (function | 0x80) {
        if response.len() != 2 { return Err("Invalid Modbus exception length".into()); }
        return Err(format!("Modbus exception: FC {function:02X}, code {:02X}", response[1]));
    }
    if response_function != function { return Err("Modbus response function mismatch".into()); }
    let address = u16::from_be_bytes([request[1], request[2]]);
    let quantity = u16::from_be_bytes([request[3], request[4]]) as usize;
    let mut result = json!({"function":function,"data_hex":hex(&response[1..])});
    match function {
        1..=4 => {
            let expected = if function <= 2 { quantity.div_ceil(8) } else { quantity * 2 };
            if response.len() != expected + 2 || response[1] as usize != expected {
                return Err(format!("Modbus response byte count mismatch: expected {expected}"));
            }
            if function <= 2 {
                result["bits"] = json!((0..quantity).map(|i| response[2 + i / 8] & (1 << (i % 8)) != 0).collect::<Vec<_>>());
            } else {
                result["registers"] = json!(response[2..].chunks_exact(2).map(|word| u16::from_be_bytes([word[0], word[1]])).collect::<Vec<_>>());
            }
        }
        5 | 6 | 15 | 16 => {
            if response != &request[..5] { return Err("Modbus write acknowledgement mismatch".into()); }
            result["write_ack"] = json!({"address":address,"quantity":if function <= 6 { 1 } else { quantity }});
        }
        _ => return Err("Unsupported Modbus response function".into()),
    }
    Ok(result)
}

fn parse_modbus_tcp_response(request: &[u8], response: &[u8]) -> Result<Value, String> {
    if response.len() < 8 || response.len() > 260 { return Err("Invalid Modbus TCP length".into()); }
    if response[..4] != request[..4] || response[2..4] != [0, 0] || response[6] != request[6] {
        return Err("Modbus transaction/protocol/unit mismatch".into());
    }
    if u16::from_be_bytes([response[4], response[5]]) as usize + 6 != response.len() {
        return Err("Modbus MBAP length mismatch".into());
    }
    parse_modbus_response(&request[7..], &response[7..])
}

fn modbus_tcp_blocking(req: ModbusRequest) -> Result<Value, String> {
    let pdu = modbus_pdu(req.function, req.address, req.quantity, req.values.as_deref())?;
    let transaction = MODBUS_TRANSACTION.fetch_add(1, Ordering::Relaxed) as u16;
    let mut frame = transaction.to_be_bytes().to_vec();
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
    frame.push(req.unit_id);
    frame.extend_from_slice(&pdu);
    let peer = resolve(&req.host, req.port)?;
    let started = Instant::now();
    let deadline = started + timeout(req.timeout_ms);
    let mut stream = TcpStream::connect_timeout(&peer, remaining(deadline)?).map_err(|error| error.to_string())?;
    stream.set_write_timeout(Some(remaining(deadline)?)).map_err(|error| error.to_string())?;
    stream.write_all(&frame).map_err(|error| error.to_string())?;
    let received = read_modbus_frame(|buffer, wait| {
        stream.set_read_timeout(Some(wait))?;
        stream.read(buffer)
    }, deadline, false).map_err(|error| format!("{error}; TX={}", hex(&frame)))?;
    let parsed = parse_modbus_tcp_response(&frame, &received)
        .map_err(|error| format!("{error}; TX={}; RX={}", hex(&frame), hex(&received)))?;
    let mut result = exchange_result(&frame, &received, started.elapsed(), peer.to_string(), "utf-8");
    result.as_object_mut().unwrap().extend(parsed.as_object().unwrap().clone());
    result["protocol"] = json!("Modbus TCP");
    result["unit_id"] = json!(req.unit_id);
    Ok(result)
}

#[tauri::command]
pub async fn protocol_modbus_tcp(req: ModbusRequest) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || modbus_tcp_blocking(req)).await.map_err(|error| format!("Modbus TCP task failed: {error}"))?
}

fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc = 0xFFFF_u16;
    for byte in data {
        crc ^= *byte as u16;
        for _ in 0..8 { crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 }; }
    }
    crc
}

fn modbus_serial_builder(port: &str, baud: u32, data_bits: Option<u8>, stop_bits: Option<u8>, parity: Option<&str>, wait: Duration) -> Result<serialport::SerialPortBuilder, String> {
    if data_bits.unwrap_or(8) != 8 { return Err("Modbus RTU requires 8 data bits".into()); }
    if baud == 0 { return Err("Invalid baud rate".into()); }
    let stop = match stop_bits.unwrap_or(1) { 1 => serialport::StopBits::One, 2 => serialport::StopBits::Two, _ => return Err("Invalid stop bits".into()) };
    let parity = match parity.unwrap_or("none").to_ascii_lowercase().as_str() {
        "none" => serialport::Parity::None, "odd" => serialport::Parity::Odd, "even" => serialport::Parity::Even,
        _ => return Err("Invalid parity".into()),
    };
    Ok(serialport::new(port, baud).data_bits(serialport::DataBits::Eight).stop_bits(stop).parity(parity).timeout(wait))
}

fn parse_modbus_rtu_response(request: &[u8], response: &[u8]) -> Result<Value, String> {
    if !(5..=256).contains(&response.len()) { return Err("Invalid Modbus RTU response length".into()); }
    let body = response.len() - 2;
    if crc16_modbus(&response[..body]) != u16::from_le_bytes([response[body], response[body + 1]]) {
        return Err("Modbus RTU CRC mismatch".into());
    }
    if response[0] != request[0] { return Err("Modbus RTU unit mismatch".into()); }
    parse_modbus_response(&request[1..request.len() - 2], &response[1..body])
}

fn drain_rtu_requests(buffer: &mut Vec<u8>, incoming: &[u8]) -> Vec<Vec<u8>> {
    buffer.extend_from_slice(incoming);
    let mut frames = Vec::new();
    while buffer.len() >= 2 {
        let length = match buffer[1] {
            1..=6 => 8,
            15 | 16 => {
                if buffer.len() < 7 { break; }
                9 + buffer[6] as usize
            }
            _ => { buffer.clear(); break; }
        };
        if length > 256 { buffer.clear(); break; }
        if buffer.len() < length { break; }
        let body = length - 2;
        if crc16_modbus(&buffer[..body]) != u16::from_le_bytes([buffer[body], buffer[body + 1]]) {
            buffer.clear();
            break;
        }
        frames.push(buffer.drain(..length).collect());
    }
    frames
}

fn modbus_rtu_server_response(packet: &[u8], unit: u8, memory: &Arc<Mutex<ModbusMemory>>) -> Result<Vec<u8>, String> {
    if packet.len() < 4 || (packet[0] != unit && packet[0] != 0) { return Ok(Vec::new()); }
    let body = packet.len() - 2;
    if crc16_modbus(&packet[..body]) != u16::from_le_bytes([packet[body], packet[body + 1]]) { return Err("Invalid RTU CRC".into()); }
    if packet[0] == 0 && !matches!(packet[1], 5 | 6 | 15 | 16) { return Ok(Vec::new()); }
    let mut tcp = vec![0, 0, 0, 0];
    tcp.extend_from_slice(&(body as u16).to_be_bytes());
    tcp.extend_from_slice(&packet[..body]);
    let response = modbus_server_response(&tcp, memory)?;
    if packet[0] == 0 { return Ok(Vec::new()); }
    let mut response = response[6..].to_vec();
    response.extend_from_slice(&crc16_modbus(&response).to_le_bytes());
    Ok(response)
}

#[tauri::command]
pub async fn protocol_modbus_rtu(req: ModbusRtuRequest) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || {
        if !(1..=247).contains(&req.unit_id) { return Err("RTU client unit must be 1-247; broadcast is not supported".into()); }
        let mut frame = vec![req.unit_id];
        frame.extend_from_slice(&modbus_pdu(req.function, req.address, req.quantity, req.values.as_deref())?);
        frame.extend_from_slice(&crc16_modbus(&frame).to_le_bytes());
        let wait = timeout(req.timeout_ms);
        let builder = modbus_serial_builder(&req.port_name, req.baud_rate, req.data_bits, req.stop_bits, req.parity.as_deref(), wait)?;
        let mut port = builder.open().map_err(|error| format!("打开串口 {} 失败：{error}", req.port_name))?;
        let started = Instant::now();
        let deadline = started + wait;
        port.write_all(&frame).map_err(|error| format!("串口发送失败：{error}"))?;
        let received = read_modbus_frame(|buffer, wait| {
            port.set_timeout(wait).map_err(std::io::Error::other)?;
            port.read(buffer)
        }, deadline, true).map_err(|error| format!("{error}; TX={}", hex(&frame)))?;
        let parsed = parse_modbus_rtu_response(&frame, &received)
            .map_err(|error| format!("{error}; TX={}; RX={}", hex(&frame), hex(&received)))?;
        let mut result = exchange_result(&frame, &received, started.elapsed(), req.port_name, "utf-8");
        result.as_object_mut().unwrap().extend(parsed.as_object().unwrap().clone());
        result["protocol"] = json!("Modbus RTU");
        result["unit_id"] = json!(req.unit_id);
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
    if !(8..=260).contains(&frame.len()) { return Err("Invalid Modbus request length".into()); }
    if frame[2..4] != [0, 0] || u16::from_be_bytes([frame[4], frame[5]]) as usize + 6 != frame.len() {
        return Err("Invalid Modbus MBAP header".into());
    }
    let function = frame[7];
    if !matches!(function, 1..=6 | 15 | 16) { return Ok(modbus_exception(frame, function, 1)); }
    if (function <= 6 && frame.len() != 12) || (function >= 15 && frame.len() < 13) {
        return Ok(modbus_exception(frame, function, 3));
    }
    let address = frame.get(8..10).map(|v| u16::from_be_bytes([v[0], v[1]]) as usize).unwrap_or(0);
    let quantity = frame.get(10..12).map(|v| u16::from_be_bytes([v[0], v[1]]) as usize).unwrap_or(0);
    // Validate the entire write before locking or mutating the shared data areas.
    if matches!(function, 1..=4 | 15 | 16) {
        let limit = match function { 1 | 2 => 2000, 3 | 4 => 125, 15 => 1968, _ => 123 };
        if quantity == 0 || quantity > limit { return Ok(modbus_exception(frame, function, 3)); }
        if address + quantity > 65_536 { return Ok(modbus_exception(frame, function, 2)); }
    }
    if function == 15 || function == 16 {
        let expected = if function == 15 { quantity.div_ceil(8) } else { quantity * 2 };
        if frame[12] as usize != expected || frame.len() != 13 + expected {
            return Ok(modbus_exception(frame, function, 3));
        }
    }
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
        let unit = req.unit_id.unwrap_or(1);
        if !(1..=247).contains(&unit) { return Err("RTU server unit must be 1-247".into()); }
        let builder = modbus_serial_builder(&port_name, req.baud_rate.unwrap_or(9_600), req.data_bits, req.stop_bits, req.parity.as_deref(), wait)?;
        let mut serial = builder.open().map_err(|error| format!("打开 RTU Server 串口 {port_name} 失败：{error}"))?;
        insert_server(&state, &kind, port_name.clone(), running.clone(), requests.clone(), clients.clone(), client_stats.clone())?;
        let memory = state.memory.clone();
        let thread_kind = kind.clone();
        let thread_running = running.clone();
        let thread_text_encoding = text_encoding.clone();
        std::thread::spawn(move || {
            let _ = app.emit("protocol-server:status", json!({"kind":thread_kind,"running":true,"address":port_name}));
            let mut received = vec![0_u8; 256];
            let mut frame_buffer = Vec::new();
            while thread_running.load(Ordering::Relaxed) {
                let size = match serial.read(&mut received) {
                    Ok(size) => size,
                    Err(error) if error.kind() == std::io::ErrorKind::TimedOut => { frame_buffer.clear(); continue; }
                    Err(_) => break,
                };
                for packet in drain_rtu_requests(&mut frame_buffer, &received[..size]) {
                    if packet[0] != unit && packet[0] != 0 { continue; }
                    requests.fetch_add(1, Ordering::Relaxed);
                    let sent = modbus_rtu_server_response(&packet, unit, &memory).unwrap_or_default();
                    if !sent.is_empty() { let _ = serial.write_all(&sent); }
                    emit_server_event(&app, &thread_kind, SocketAddr::from(([127,0,0,1],0)), &packet, &sent, &thread_text_encoding, "rtu");
                }
            }
            thread_running.store(false, Ordering::Relaxed);
            let _ = app.emit("protocol-server:status", json!({"kind":thread_kind,"running":false}));
        });
    } else if kind.ends_with("tcp") || kind == "tcp" {
        let framing = if kind == "modbus_tcp" { TcpFraming::ModbusTcp } else { tcp_framing(req.framing_mode.as_deref(), req.frame_delimiter_hex.as_deref(), req.fixed_frame_length, "raw")? };
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

    fn tcp_adu(pdu: &[u8]) -> Vec<u8> {
        let mut frame = vec![0, 7, 0, 0];
        frame.extend_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
        frame.push(1);
        frame.extend_from_slice(pdu);
        frame
    }

    fn rtu_adu(unit: u8, pdu: &[u8]) -> Vec<u8> {
        let mut frame = vec![unit];
        frame.extend_from_slice(pdu);
        frame.extend_from_slice(&crc16_modbus(&frame).to_le_bytes());
        frame
    }

    #[test]
    fn modbus_rejects_invalid_requests_without_clamping() {
        assert_eq!(modbus_pdu(5, 0, Some(1), Some(&[1])).unwrap(), [5, 0, 0, 255, 0]);
        assert_eq!(modbus_pdu(5, 0, None, Some(&[0xFF00])).unwrap(), [5, 0, 0, 255, 0]);
        assert_eq!(modbus_pdu(5, 0, None, Some(&[0])).unwrap(), [5, 0, 0, 0, 0]);
        assert_eq!(modbus_pdu(15, 0, None, Some(&[1, 0, 1])).unwrap(), [15, 0, 0, 0, 3, 1, 5]);
        for (fc, address, count, values) in [
            (3, 0, 126, vec![]), (1, 0, 0, vec![]), (1, 65535, 2, vec![]),
            (5, 0, 1, vec![2]), (6, 0, 1, vec![]), (6, 0, 1, vec![1, 2]),
            (15, 0, 2, vec![1]), (15, 0, 1, vec![2]), (16, 65535, 2, vec![1, 2]),
        ] { assert!(modbus_pdu(fc, address, Some(count), Some(&values)).is_err()); }
    }

    #[test]
    fn modbus_response_identity_lengths_and_echo_are_strict() {
        let request = tcp_adu(&[1, 0, 0, 0, 24]);
        let response = tcp_adu(&[1, 3, 1, 0, 0]);
        assert_eq!(parse_modbus_tcp_response(&request, &response).unwrap()["bits"].as_array().unwrap().len(), 24);
        assert!(parse_modbus_tcp_response(&request, &tcp_adu(&[1, 1, 0])).is_err());
        for index in [0, 2, 4, 6, 7, 8] {
            let mut invalid = response.clone();
            invalid[index] ^= 1;
            assert!(parse_modbus_tcp_response(&request, &invalid).is_err(), "index {index}");
        }
        for length in 0..response.len() { assert!(parse_modbus_tcp_response(&request, &response[..length]).is_err()); }
        assert!(parse_modbus_response(&[3, 0, 0, 0, 1], &[3, 4, 0, 1, 0, 2]).is_err());
        for fc in [5, 6, 15, 16] {
            let request = modbus_pdu(fc, 8, Some(1), Some(&[1])).unwrap();
            assert!(parse_modbus_response(&request, &request[..5]).is_ok());
            let mut wrong = request[..5].to_vec();
            wrong[4] ^= 1;
            assert!(parse_modbus_response(&request, &wrong).is_err());
            assert!(parse_modbus_response(&request, &[fc | 0x80, 2]).unwrap_err().contains("exception"));
        }
    }

    #[test]
    fn malformed_server_writes_do_not_mutate_or_poison_memory() {
        let memory = ProtocolServerState::default().memory;
        for pdu in [vec![15, 0, 0, 0, 24, 1, 255], vec![16, 0, 0, 0, 2, 2, 0, 42], vec![5, 0, 0, 0, 1], vec![6, 0, 0], vec![3, 0, 0, 0, 126]] {
            let response = modbus_server_response(&tcp_adu(&pdu), &memory).unwrap();
            assert_eq!(response[7], pdu[0] | 0x80);
            assert_eq!(response[8], 3);
            let data = memory.lock().unwrap();
            assert!(!data.coils[0]);
            assert_eq!(data.holding_registers[0], 0);
        }
        let valid = tcp_adu(&modbus_pdu(5, 2, None, Some(&[1])).unwrap());
        assert_eq!(modbus_server_response(&valid, &memory).unwrap(), valid);
        assert!(memory.lock().unwrap().coils[2]);
        let mut invalid = valid.clone();
        invalid[5] -= 1;
        assert!(modbus_server_response(&invalid, &memory).is_err());
        let mut buffer = Vec::new();
        assert!(drain_tcp_frames(&mut buffer, &[0, 1, 0, 0, 255, 255, 1], &TcpFraming::ModbusTcp).is_empty());
        assert!(buffer.is_empty());
    }

    #[test]
    fn modbus_frame_reader_handles_byte_fragments_eof_and_deadline() {
        for (frame, rtu) in [(tcp_adu(&[3, 2, 0, 42]), false), (rtu_adu(1, &[3, 2, 0, 42]), true), (rtu_adu(1, &[0x83, 2]), true)] {
            let mut cursor = std::io::Cursor::new(frame.clone());
            let received = read_modbus_frame(|buffer, _| cursor.read(&mut buffer[..1]), Instant::now() + Duration::from_secs(1), rtu).unwrap();
            assert_eq!(received, frame);
        }
        assert!(read_modbus_frame(|_, _| Ok(0), Instant::now() + Duration::from_secs(1), false).is_err());
        assert!(read_modbus_frame(|_, _| panic!("must not read"), Instant::now(), false).is_err());
    }

    #[test]
    fn modbus_tcp_real_socket_fragmentation_and_timeout() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut request = [0; 12];
            stream.read_exact(&mut request).unwrap();
            let mut response = tcp_adu(&[3, 2, 0x12, 0x34]);
            response[..2].copy_from_slice(&request[..2]);
            for byte in response { stream.write_all(&[byte]).unwrap(); std::thread::sleep(Duration::from_millis(2)); }
        });
        let result = modbus_tcp_blocking(ModbusRequest { host:"127.0.0.1".into(), port, unit_id:1, function:3, address:0, quantity:Some(1), values:None, timeout_ms:Some(1000) });
        server.join().unwrap();
        assert_eq!(result.unwrap()["registers"], json!([0x1234]));

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut request = [0; 12];
            stream.read_exact(&mut request).unwrap();
            for byte in [0, 1, 0, 0, 0, 5] {
                if stream.write_all(&[byte]).is_err() { break; }
                std::thread::sleep(Duration::from_millis(60));
            }
        });
        let started = Instant::now();
        let result = modbus_tcp_blocking(ModbusRequest { host:"127.0.0.1".into(), port, unit_id:1, function:3, address:0, quantity:Some(1), values:None, timeout_ms:Some(100) });
        let elapsed = started.elapsed();
        server.join().unwrap();
        assert!(result.is_err());
        assert!(elapsed < Duration::from_millis(300), "{elapsed:?}");
    }

    #[test]
    fn rtu_validates_crc_identity_and_server_broadcasts() {
        let request = rtu_adu(1, &[3, 0, 0, 0, 1]);
        let response = rtu_adu(1, &[3, 2, 0, 42]);
        assert_eq!(parse_modbus_rtu_response(&request, &response).unwrap()["registers"], json!([42]));
        assert!(parse_modbus_rtu_response(&request, &rtu_adu(2, &[3, 2, 0, 42])).is_err());
        assert!(parse_modbus_rtu_response(&request, &rtu_adu(1, &[4, 2, 0, 42])).is_err());
        assert!(parse_modbus_rtu_response(&rtu_adu(1, &[1, 0, 0, 0, 24]), &rtu_adu(1, &[1, 1, 0])).is_err());
        let mut invalid = response;
        invalid[3] ^= 1;
        assert!(parse_modbus_rtu_response(&request, &invalid).is_err());
        let mut buffer = Vec::new();
        assert!(drain_rtu_requests(&mut buffer, &request[..3]).is_empty());
        let tail = [&request[3..], &request].concat();
        assert_eq!(drain_rtu_requests(&mut buffer, &tail), vec![request.clone(), request]);
        let memory = ProtocolServerState::default().memory;
        assert!(modbus_rtu_server_response(&rtu_adu(2, &[6, 0, 0, 0, 7]), 1, &memory).unwrap().is_empty());
        assert_eq!(memory.lock().unwrap().holding_registers[0], 0);
        assert!(modbus_rtu_server_response(&rtu_adu(0, &[6, 0, 0, 0, 7]), 1, &memory).unwrap().is_empty());
        assert_eq!(memory.lock().unwrap().holding_registers[0], 7);
        assert!(!modbus_rtu_server_response(&rtu_adu(1, &[3, 0, 0, 0, 1]), 1, &memory).unwrap().is_empty());
    }
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
