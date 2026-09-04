#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit = "512"]

use serde_json::{json, Value};
use std::net::IpAddr;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, System};
use tauri::Manager;

mod ai;
mod audits;
mod camera;
mod capture;
mod dhcp;
mod diagnostics;
mod discovery;
mod firewall;
mod interfaces;
mod ip_conflict;
mod ipinfo;
mod lan_speed;
mod link_monitor;
mod lldp;
mod local_settings;
mod net_services;
mod ping;
mod portscan;
mod protocol_workbench;
mod remote;
mod routes;
mod serial;
mod session_test;
mod skills;
mod speed;
mod subnet;
mod terminal;
mod tracert;
mod traffic;

fn local_ip() -> String {
    local_ip_address::local_ip()
        .unwrap_or(IpAddr::from([127, 0, 0, 1]))
        .to_string()
}

fn network_identity() -> (String, String, Vec<String>) {
    let primary = interfaces::all_interfaces().ok().and_then(|items| {
        items
            .into_iter()
            .find(|item| item["is_lan_primary"] == true)
    });
    if let Some(item) = primary {
        let ip = item["ipv4"].as_str().unwrap_or("127.0.0.1").to_string();
        let gateway = item["gateway"].as_str().unwrap_or("-").to_string();
        let dns = item["dns"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect();
        (ip, gateway, dns)
    } else {
        (local_ip(), "-".into(), Vec::new())
    }
}

fn dashboard_traffic() -> (f64, f64) {
    static NETWORKS: OnceLock<Mutex<Networks>> = OnceLock::new();
    let networks = NETWORKS.get_or_init(|| Mutex::new(Networks::new_with_refreshed_list()));
    let Ok(mut networks) = networks.lock() else {
        return (0.0, 0.0);
    };
    networks.refresh();
    let received = networks
        .iter()
        .map(|(_, data)| data.received())
        .sum::<u64>() as f64
        / 1024.0;
    let transmitted = networks
        .iter()
        .map(|(_, data)| data.transmitted())
        .sum::<u64>() as f64
        / 1024.0;
    (received, transmitted)
}

#[tauri::command]
fn license_status() -> Value {
    json!({
        "is_activated": true,
        "license_type": "community",
        "expire_at": "永久",
        "hwid": "REBUILD-LOCAL",
        "message": "本地重实现版本"
    })
}

#[tauri::command]
fn license_verify() -> bool {
    true
}

#[tauri::command]
fn license_activate() -> Value {
    license_status()
}

#[tauri::command]
fn license_import() -> Value {
    license_status()
}

#[tauri::command]
fn license_deactivate() -> bool {
    true
}

#[tauri::command]
fn app_info() -> Value {
    json!({
        "name": "网络测试工具箱",
        "version": "9.0.0-rebuild",
        "author": "运维杂谈",
        "description": "网络诊断、扫描、监控与运维工具集合",
        "platform": "windows",
        "arch": "x86_64"
    })
}

#[tauri::command]
async fn get_elevation_status() -> Result<Value, String> {
    let elevated = tokio::task::spawn_blocking(firewall::elevated)
        .await
        .map_err(|error| format!("读取管理员权限任务失败：{error}"))?;
    Ok(json!({ "elevated": elevated, "is_elevated": elevated, "is_admin": elevated }))
}

fn dashboard_snapshot_blocking() -> Value {
    let mut system = System::new_all();
    system.refresh_all();
    std::thread::sleep(Duration::from_millis(100));
    system.refresh_cpu();
    let (ip, gateway, dns) = network_identity();
    let primary_dns = dns.first().cloned().unwrap_or_else(|| "-".into());
    let (network_recv_kbps, network_sent_kbps) = dashboard_traffic();
    let host = System::host_name().unwrap_or_else(|| "Windows".into());
    let uptime = System::uptime();
    let memory_percent = if system.total_memory() == 0 {
        0.0
    } else {
        system.used_memory() as f64 * 100.0 / system.total_memory() as f64
    };
    let memory_total_gb = system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let memory_used_gb = system.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let disks = Disks::new_with_refreshed_list();
    let disk_total = disks
        .list()
        .iter()
        .map(|disk| disk.total_space())
        .sum::<u64>();
    let disk_available = disks
        .list()
        .iter()
        .map(|disk| disk.available_space())
        .sum::<u64>();
    let disk_used = disk_total.saturating_sub(disk_available);
    let disk_total_gb = disk_total as f64 / 1024.0 / 1024.0 / 1024.0;
    let disk_used_gb = disk_used as f64 / 1024.0 / 1024.0 / 1024.0;
    let disk_percent = if disk_total == 0 {
        0.0
    } else {
        disk_used as f64 * 100.0 / disk_total as f64
    };
    let days = uptime / 86_400;
    let hours = (uptime % 86_400) / 3_600;
    let minutes = (uptime % 3_600) / 60;
    json!({
        "greeting": "欢迎回来",
        "os": "Windows",
        "hostname": host,
        "local_ip": ip,
        "gateway_ip": gateway,
        "dns_servers": dns,
        "os_name": System::name().unwrap_or_else(|| "Windows".into()),
        "os_version": System::long_os_version().unwrap_or_else(|| "Windows".into()),
        "uptime_seconds": uptime,
        "uptime_text": format!("{days} 天 {hours} 小时 {minutes} 分"),
        "process_count": system.processes().len(),
        "cpu_usage": system.global_cpu_info().cpu_usage(),
        "cpu_cores": system.cpus().len(),
        "memory_total_gb": memory_total_gb,
        "memory_used_gb": memory_used_gb,
        "memory_percent": memory_percent,
        "disk_total_gb": disk_total_gb,
        "disk_used_gb": disk_used_gb,
        "disk_percent": disk_percent,
        "network_recv_kbps": network_recv_kbps,
        "network_sent_kbps": network_sent_kbps,
        "connectivity": {
            "local": { "state": "ok", "ok": true, "label": "本机", "address": local_ip(), "latency_ms": 0 },
            "gateway": { "state": "pending", "ok": false, "label": "网关", "address": gateway, "latency_ms": null },
            "dns": { "state": "pending", "ok": false, "label": "DNS", "address": primary_dns, "latency_ms": null },
            "internet": { "state": "pending", "ok": false, "label": "互联网", "address": "www.baidu.com", "latency_ms": null }
        }
    })
}

#[tauri::command]
async fn get_dashboard_snapshot() -> Result<Value, String> {
    tokio::task::spawn_blocking(dashboard_snapshot_blocking)
        .await
        .map_err(|error| format!("采集系统状态任务失败：{error}"))
}

async fn tcp_latency(host: &str, port: u16) -> Option<u128> {
    let started = Instant::now();
    let address = format!("{host}:{port}");
    match tokio::time::timeout(
        Duration::from_millis(1200),
        tokio::net::TcpStream::connect(address),
    )
    .await
    {
        Ok(Ok(_)) => Some(started.elapsed().as_millis()),
        _ => None,
    }
}

#[tauri::command]
async fn get_dashboard_connectivity() -> Value {
    let (_, gateway, dns) = tokio::task::spawn_blocking(network_identity)
        .await
        .unwrap_or_else(|_| ("127.0.0.1".into(), "127.0.0.1".into(), vec!["1.1.1.1".into()]));
    let primary_dns = dns.first().cloned().unwrap_or_else(|| "1.1.1.1".into());
    let gateway_started = Instant::now();
    let gateway_ms = tokio::process::Command::new("ping.exe")
        .args(["-n", "1", "-w", "1200", &gateway])
        .creation_flags(0x08000000)
        .output()
        .await
        .ok()
        .filter(|out| out.status.success())
        .map(|_| gateway_started.elapsed().as_millis());
    let dns_ms = tcp_latency(&primary_dns, 53).await;
    let internet_ms = tcp_latency("www.baidu.com", 443).await;
    let node = |label: &str, address: String, latency: Option<u128>| {
        json!({
            "state": if latency.is_some() { "ok" } else { "bad" },
            "ok": latency.is_some(),
            "label": label,
            "address": address,
            "latency_ms": latency
        })
    };
    json!({
        "local": node("本机", local_ip(), Some(0)),
        "gateway": node("网关", gateway, gateway_ms),
        "dns": node("DNS", primary_dns, dns_ms),
        "internet": node("互联网", "www.baidu.com".into(), internet_ms)
    })
}

#[tauri::command]
fn relaunch_as_admin() -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let escaped = exe.to_string_lossy().replace('\'', "''");
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("Start-Process -FilePath '{escaped}' -Verb RunAs"),
        ])
        .creation_flags(0x08000000)
        .status()
        .map_err(|error| format!("请求管理员权限失败：{error}"))?;
    if status.success() {
        Ok(true)
    } else {
        Err("管理员权限请求被取消".into())
    }
}

fn main() {
    tauri::Builder::default()
        .manage(ping::PingState::default())
        .manage(discovery::DiscoveryState::default())
        .manage(portscan::PortScanState::default())
        .manage(tracert::TracertState::default())
        .manage(traffic::TrafficState::default())
        .manage(serial::SerialState::default())
        .manage(protocol_workbench::ProtocolServerState::default())
        .manage(protocol_workbench::ProtocolClientState::default())
        .manage(terminal::TerminalState::default())
        .manage(capture::CaptureState::default())
        .manage(camera::CameraState::default())
        .manage(link_monitor::LinkMonitorState::default())
        .manage(speed::SpeedState::default())
        .manage(ip_conflict::ConflictState::default())
        .manage(net_services::NetServiceState::default())
        .manage(session_test::SessionTestState::default())
        .manage(lan_speed::LanSpeedState::default())
        .manage(lldp::LldpState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if std::env::args().any(|arg| arg == "--open-protocol-workbench") {
                if let Some(webview) = app.get_webview_window("main") {
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(1_500));
                        if let Err(error) = webview.eval("location.hash = '#/protocol-workbench'") {
                            eprintln!("protocol workbench verification navigation failed: {error}");
                        }
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            license_status,
            license_verify,
            license_activate,
            license_import,
            license_deactivate,
            app_info,
            get_elevation_status,
            get_dashboard_snapshot,
            get_dashboard_connectivity,
            relaunch_as_admin,
            subnet::calc_subnet_ipv4,
            subnet::calc_subnet_ipv6,
            subnet::calc_wildcard_mask,
            subnet::split_subnets,
            subnet::summarize_routes,
            subnet::calc_vlsm,
            ping::start_ping_once,
            ping::start_continuous_ping,
            ping::start_tcp_ping,
            ping::stop_ping,
            ping::start_batch_ping,
            routes::add_route,
            ai::ai_analyze_packet_capture,
            ai::ai_chat,
            camera::camera_build_rtsp_url,
            camera::camera_change_password,
            camera::camera_get_device_info,
            camera::camera_get_snapshot,
            camera::camera_inspect_firmware,
            camera::camera_probe_auth,
            camera::camera_start_batch_upgrade,
            camera::camera_start_live_stream,
            camera::camera_stop_batch_upgrade,
            camera::camera_stop_live_stream,
            capture::capture_status,
            lan_speed::check_iperf3,
            capture::clear_packet_capture,
            routes::delete_route,
            capture::export_packet_capture,
            firewall::firewall_add_port_rule,
            firewall::firewall_snapshot,
            firewall::firewall_set_all,
            firewall::firewall_set_profile,
            firewall::firewall_set_rule_enabled,
            firewall::firewall_delete_rule,
            capture::import_packet_capture,
            remote::launch_rdp,
            link_monitor::link_monitor_running,
            camera::list_camera_vendors,
            capture::list_capture_ifaces,
            interfaces::list_conflict_ifaces,
            interfaces::list_dhcp_ifaces,
            interfaces::list_lan_ifaces,
            interfaces::list_lldp_ifaces,
            lldp::list_lldp_neighbors,
            interfaces::list_loop_interfaces,
            routes::list_route_table,
            interfaces::list_segment_ifaces,
            interfaces::list_traffic_ifaces,
            ipinfo::list_wifi_interfaces,
            ipinfo::list_wifi_passwords,
            lldp::lldp_status,
            local_settings::local_apply_ip_config,
            local_settings::local_get_ip_config,
            interfaces::local_list_ip_ifaces,
            local_settings::local_open_tool,
            local_settings::local_run_action,
            local_settings::local_run_repair,
            local_settings::local_settings_snapshot,
            ipinfo::lookup_mac_vendor,
            net_services::net_svc_smoke_test,
            net_services::net_svc_start_dhcp,
            net_services::net_svc_start_ftp,
            net_services::net_svc_start_http,
            net_services::net_svc_start_radius,
            net_services::net_svc_start_syslog,
            net_services::net_svc_start_tftp,
            net_services::net_svc_stop_dhcp,
            net_services::net_svc_stop_ftp,
            net_services::net_svc_stop_http,
            net_services::net_svc_stop_radius,
            net_services::net_svc_stop_syslog,
            net_services::net_svc_stop_tftp,
            net_services::net_svc_status,
            diagnostics::protocol_analyze_live,
            diagnostics::protocol_analyze_snapshot,
            traffic::reset_traffic_baseline,
            diagnostics::run_chain_conn_test,
            diagnostics::run_site_conn_test,
            traffic::sample_traffic,
            skills::send_feishu_webhook,
            remote::send_wake_on_lan,
            skills::send_wecom_webhook,
            serial::serial_close,
            serial::serial_list_ports,
            serial::serial_open,
            serial::serial_write,
            skills::skill_execute,
            skills::skill_list,
            skills::skill_prompt,
            camera::start_camera_scan,
            dhcp::start_dhcp_detect,
            audits::start_health_check,
            discovery::start_host_discovery,
            ip_conflict::start_ip_conflict,
            lan_speed::start_lan_server,
            lan_speed::start_lan_speed_test,
            link_monitor::start_link_monitor,
            lldp::start_lldp_scan,
            audits::start_log_audit,
            audits::start_loop_detection,
            capture::start_packet_capture,
            portscan::start_port_scan,
            protocol_workbench::protocol_modbus_rtu,
            protocol_workbench::protocol_modbus_tcp,
            protocol_workbench::protocol_payload_preview,
            protocol_workbench::protocol_server_get_memory,
            protocol_workbench::protocol_server_set_memory,
            protocol_workbench::protocol_server_send,
            protocol_workbench::protocol_server_start,
            protocol_workbench::protocol_server_status,
            protocol_workbench::protocol_server_stop,
            protocol_workbench::protocol_server_stop_all,
            protocol_workbench::protocol_tcp_exchange,
            protocol_workbench::protocol_tcp_client_connect,
            protocol_workbench::protocol_tcp_client_disconnect,
            protocol_workbench::protocol_tcp_client_send,
            protocol_workbench::protocol_tcp_client_status,
            protocol_workbench::protocol_tcp_loopback_test,
            protocol_workbench::protocol_udp_exchange,
            protocol_workbench::protocol_workbench_catalog,
            audits::start_security_test,
            ping::start_segment_ping,
            session_test::start_session_test,
            speed::start_speed_test,
            tracert::start_tracert,
            camera::stop_camera_scan,
            discovery::stop_host_discovery,
            ip_conflict::stop_ip_conflict,
            lan_speed::stop_lan_server,
            lan_speed::stop_lan_speed_test,
            link_monitor::stop_link_monitor,
            lldp::stop_lldp_scan,
            capture::stop_packet_capture,
            portscan::stop_port_scan,
            session_test::stop_session_test,
            speed::stop_speed_test,
            tracert::stop_tracert,
            terminal::term_session_close,
            terminal::term_session_close_all,
            terminal::term_session_ssh_open,
            terminal::term_session_telnet_open,
            terminal::term_session_write,
            diagnostics::tools_conn_stats,
            diagnostics::tools_conn_stats_detail,
            diagnostics::tools_dns_hijack,
            diagnostics::tools_dns_lookup,
            diagnostics::tools_dns_prefer,
            diagnostics::tools_mtu_test,
            diagnostics::tools_nslookup,
            diagnostics::tools_port_process,
            diagnostics::tools_ssl_check,
            diagnostics::tools_telnet_test,
            local_settings::write_text_file,
            interfaces::get_local_ip_info,
            ipinfo::convert_mac_format,
            ipinfo::get_public_ip_info,
            ipinfo::list_arp_table,
            ipinfo::query_ip_info,
            ipinfo::scan_wifi_networks
        ])
        .run(tauri::generate_context!())
        .expect("network toolbox runtime failed");
}
