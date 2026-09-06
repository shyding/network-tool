use serde_json::{json, Value};
use std::io::{Read, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct SerialState {
    port: Mutex<Option<Box<dyn serialport::SerialPort>>>,
    running: Arc<AtomicBool>,
}

#[tauri::command(async)]
pub fn serial_list_ports() -> Result<Value, String> {
    let ports = serialport::available_ports().map_err(|e| format!("枚举串口失败：{e}"))?;
    Ok(json!(ports
        .into_iter()
        .map(|p| {
            let detail = match p.port_type {
                serialport::SerialPortType::UsbPort(info) => format!(
                    "USB {:04X}:{:04X} {}",
                    info.vid,
                    info.pid,
                    info.product.unwrap_or_default()
                ),
                serialport::SerialPortType::BluetoothPort => "Bluetooth".into(),
                serialport::SerialPortType::PciPort => "PCI".into(),
                serialport::SerialPortType::Unknown => "串口".into(),
            };
            json!({"name":p.port_name,"description":detail})
        })
        .collect::<Vec<_>>()))
}

#[tauri::command(async)]
pub fn serial_open(
    app: AppHandle,
    state: State<'_, SerialState>,
    port: String,
    baud: u32,
    data_bits: u8,
    stop_bits: f64,
    parity: String,
) -> Result<bool, String> {
    state.running.store(false, Ordering::Relaxed);
    let _ = state.port.lock().map_err(|_| "串口状态锁异常")?.take();
    let data_bits = match data_bits {
        5 => serialport::DataBits::Five,
        6 => serialport::DataBits::Six,
        7 => serialport::DataBits::Seven,
        _ => serialport::DataBits::Eight,
    };
    let stop_bits = if stop_bits >= 1.5 {
        serialport::StopBits::Two
    } else {
        serialport::StopBits::One
    };
    let parity = match parity.to_ascii_lowercase().as_str() {
        "odd" | "奇" => serialport::Parity::Odd,
        "even" | "偶" => serialport::Parity::Even,
        _ => serialport::Parity::None,
    };
    let opened = serialport::new(&port, baud)
        .data_bits(data_bits)
        .stop_bits(stop_bits)
        .parity(parity)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| format!("打开 {port} 失败：{e}"))?;
    let mut reader = opened
        .try_clone()
        .map_err(|e| format!("创建串口读取器失败：{e}"))?;
    state.running.store(true, Ordering::Relaxed);
    let running = state.running.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while running.load(Ordering::Relaxed) {
            match reader.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let _ = app.emit(
                        "serial:data",
                        String::from_utf8_lossy(&buf[..n]).into_owned(),
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => break,
                _ => {}
            }
        }
    });
    *state.port.lock().map_err(|_| "串口状态锁异常")? = Some(opened);
    Ok(true)
}

#[tauri::command(async)]
pub fn serial_write(state: State<'_, SerialState>, data: String) -> Result<usize, String> {
    let mut guard = state.port.lock().map_err(|_| "串口状态锁异常")?;
    let port = guard.as_mut().ok_or("串口尚未打开")?;
    port.write_all(data.as_bytes())
        .map_err(|e| format!("串口写入失败：{e}"))?;
    port.flush().map_err(|e| e.to_string())?;
    Ok(data.len())
}

#[tauri::command(async)]
pub fn serial_close(state: State<'_, SerialState>) -> Result<bool, String> {
    state.running.store(false, Ordering::Relaxed);
    state.port.lock().map_err(|_| "串口状态锁异常")?.take();
    Ok(true)
}
