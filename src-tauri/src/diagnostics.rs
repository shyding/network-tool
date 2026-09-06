use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;

#[tauri::command]
pub async fn tools_dns_lookup(domain: String) -> Result<Value, String> {
    let started = Instant::now();
    let addresses = tokio::net::lookup_host((domain.as_str(), 0))
        .await
        .map_err(|error| format!("DNS 查询失败：{error}"))?;
    let mut ipv4 = BTreeSet::new();
    let mut ipv6 = BTreeSet::new();
    for address in addresses {
        match address.ip() {
            std::net::IpAddr::V4(ip) => {
                ipv4.insert(ip.to_string());
            }
            std::net::IpAddr::V6(ip) => {
                ipv6.insert(ip.to_string());
            }
        }
    }
    Ok(
        json!({"ok": true, "domain": domain, "ipv4": ipv4, "ipv6": ipv6, "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0}),
    )
}

#[tauri::command]
pub async fn tools_telnet_test(host: String, port: u16, timeout_secs: f64) -> Value {
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs_f64(timeout_secs.clamp(0.1, 60.0)),
        tokio::net::TcpStream::connect((host.as_str(), port)),
    )
    .await;
    let ok = matches!(result, Ok(Ok(_)));
    json!({"ok": ok, "host": host, "port": port,
        "latency_ms": if ok { Some(started.elapsed().as_secs_f64() * 1000.0) } else { None },
        "message": if ok { "TCP 连接成功" } else { "TCP 连接失败或超时" }})
}

#[tauri::command]
pub async fn tools_nslookup(
    domain: String,
    dns_server: Option<String>,
    qtype: String,
) -> Result<Value, String> {
    let mut command = Command::new("nslookup.exe");
    command.args([format!("-type={qtype}"), domain.clone()]);
    if let Some(server) = dns_server.as_deref().filter(|value| !value.is_empty()) {
        command.arg(server);
    }
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .output()
        .await
        .map_err(|error| error.to_string())?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).into_owned()
    } else {
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    Ok(
        json!({"ok": output.status.success(), "domain": domain, "qtype": qtype,
        "dns_server": dns_server.unwrap_or_else(|| "系统默认".into()), "output": text}),
    )
}

#[tauri::command]
pub async fn tools_conn_stats() -> Result<Value, String> {
    let detail = tools_conn_stats_detail().await?;
    Ok(
        json!({"message":"已读取系统连接表","total":detail["total"],"established":detail["rows"].as_array().map(|r|r.iter().filter(|v|v["state"]=="ESTABLISHED").count()).unwrap_or(0),"tcp":detail["rows"].as_array().map(|r|r.iter().filter(|v|v["proto"]=="TCP").count()).unwrap_or(0),"udp":detail["rows"].as_array().map(|r|r.iter().filter(|v|v["proto"]=="UDP").count()).unwrap_or(0),"unique_remote":detail["rows"].as_array().map(|r|r.iter().filter_map(|v|v["remote"].as_str()).collect::<BTreeSet<_>>().len()).unwrap_or(0),"status_stats":detail["status_stats"],"proto_stats":detail["proto_stats"]}),
    )
}

#[tauri::command]
pub async fn tools_conn_stats_detail() -> Result<Value, String> {
    let output = Command::new("netstat.exe")
        .args(["-ano"])
        .creation_flags(0x08000000)
        .output()
        .await
        .map_err(|error| error.to_string())?;
    let system = sysinfo::System::new_all();
    let rows = String::from_utf8_lossy(&output.stdout).lines().filter_map(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let proto=*fields.first()?; if proto!="TCP"&&proto!="UDP"{return None}let (state,pid)=if proto=="TCP"&&fields.len()>=5{(fields[3],fields[4])}else if proto=="UDP"&&fields.len()>=4{("LISTEN",fields[3])}else{return None};let local=fields[1];let remote=fields[2];let local_port=local.rsplit(':').next().and_then(|p|p.parse::<u16>().ok()).unwrap_or(0);let (service,category)=service_info(local_port);let process=pid.parse::<usize>().ok().and_then(|id|system.process(sysinfo::Pid::from(id))).map(|p|p.name().to_string()).unwrap_or_default();let risk=if state.starts_with("LISTEN")&&matches!(local_port,21|23|135|139|445|3389|5900){"high"}else if state.starts_with("LISTEN"){"medium"}else{"info"};
        Some(json!({"protocol":proto,"proto":proto,"local":local,"remote":remote,"local_port":local_port,"state":state,"pid":pid,"process":process,"service":service,"category":category,"risk":risk}))
    }).collect::<Vec<_>>();
    let stats = |field: &str| {
        let mut m = std::collections::BTreeMap::<String, u64>::new();
        for row in &rows {
            let key = row[field].as_str().unwrap_or("Unknown").to_string();
            *m.entry(key).or_default() += 1;
        }
        m.into_iter().collect::<Vec<_>>()
    };
    let known = rows.iter().filter(|r| r["service"] != "Unknown").count();
    Ok(
        json!({"total":rows.len(),"known_services":known,"known_service_count":known,"status_stats":stats("state"),"proto_stats":stats("proto"),"category_stats":stats("category"),"top_flows":rows.iter().take(20).cloned().collect::<Vec<_>>(),"rows":rows}),
    )
}

fn service_info(port: u16) -> (&'static str, &'static str) {
    match port {
        20 | 21 => ("FTP", "file"),
        22 => ("SSH", "remote"),
        23 => ("Telnet", "remote"),
        25 | 110 | 143 | 465 | 587 | 993 | 995 => ("Mail", "mail"),
        53 => ("DNS", "other"),
        80 | 443 | 8080 | 8443 => ("HTTP", "web"),
        135 | 139 | 445 => ("SMB/RPC", "file"),
        1433 | 3306 | 5432 | 1521 => ("Database", "database"),
        3389 => ("RDP", "remote"),
        6379 | 11211 => ("Cache", "cache"),
        554 => ("RTSP", "iot"),
        _ => ("Unknown", "other"),
    }
}

#[tauri::command]
pub async fn tools_port_process(port_filter: Option<String>) -> Result<Value, String> {
    let detail = tools_conn_stats_detail().await?;
    let mut rows = detail["rows"].as_array().cloned().unwrap_or_default();
    if let Some(port) = port_filter.filter(|value| !value.is_empty()) {
        rows.retain(|row| {
            row["local"]
                .as_str()
                .is_some_and(|value| value.ends_with(&format!(":{port}")))
        });
    }
    let count = rows.len();
    let known = rows.iter().filter(|r| r["service"] != "Unknown").count();
    Ok(json!({"rows":rows,"count":count,"known_service_count":known}))
}

#[tauri::command]
pub async fn tools_dns_prefer(domain: String, rounds: u32) -> Result<Value, String> {
    let servers = [
        ("阿里DNS (主)", "223.5.5.5"),
        ("阿里DNS (备)", "223.6.6.6"),
        ("腾讯DNS (主)", "119.29.29.29"),
        ("腾讯DNS (备)", "182.254.116.116"),
        ("114DNS (主)", "114.114.114.114"),
        ("114DNS (备)", "114.114.115.115"),
        ("百度DNS", "180.76.76.76"),
        ("OneDNS (标准)", "117.50.11.11"),
        ("OneDNS (安全)", "117.50.22.22"),
        ("DNS派 (主)", "101.226.4.6"),
        ("DNS派 (备)", "218.30.118.6"),
        ("CNNIC DNS", "52.80.52.52"),
        ("Google DNS (主)", "8.8.8.8"),
        ("Google DNS (备)", "8.8.4.4"),
        ("Cloudflare (主)", "1.1.1.1"),
        ("Cloudflare (备)", "1.0.0.1"),
        ("Quad9", "9.9.9.9"),
        ("OpenDNS (主)", "208.67.222.222"),
        ("OpenDNS (备)", "208.67.220.220"),
        ("Level3 DNS", "4.2.2.1"),
    ];
    let mut rows = Vec::new();
    for (name, ip) in servers {
        let mut samples = Vec::new();
        let mut resolved = Vec::new();
        for _ in 0..rounds.clamp(1, 10) {
            let started = Instant::now();
            let answers = nslookup_ips(&domain, Some(ip)).await;
            if !answers.is_empty() {
                samples.push(started.elapsed().as_secs_f64() * 1000.0);
                resolved = answers;
            }
        }
        let avg = (!samples.is_empty()).then(|| samples.iter().sum::<f64>() / samples.len() as f64);
        let success_rate =
            (samples.len() as f64 * 100.0 / rounds.clamp(1, 10) as f64).round() as u64;
        rows.push(json!({"name":name,"ip":ip,"avg_ms":avg,"success":samples.len(),"rounds":rounds,"success_rate":success_rate,"resolved_ip":resolved.first(),"resolved_ips":resolved,"status":if avg.is_some(){"ok"}else{"fail"}}));
    }
    rows.sort_by(|a, b| {
        a["avg_ms"]
            .as_f64()
            .unwrap_or(f64::MAX)
            .total_cmp(&b["avg_ms"].as_f64().unwrap_or(f64::MAX))
    });
    for (index, row) in rows.iter_mut().enumerate() {
        row["rank"] = json!(index + 1);
    }
    let best = rows.iter().find(|r| r["avg_ms"].is_number());
    Ok(
        json!({"domain":domain,"rows":rows,"best_ip":best.map(|r|r["ip"].clone()),"best_name":best.map(|r|r["name"].clone()),"best_avg_ms":best.and_then(|r|r["avg_ms"].as_f64())}),
    )
}

async fn nslookup_ips(domain: &str, server: Option<&str>) -> Vec<String> {
    let mut cmd = Command::new("nslookup.exe");
    cmd.arg(domain);
    if let Some(s) = server {
        cmd.arg(s);
    }
    let out = cmd.creation_flags(0x08000000).output().await.ok();
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    text.split_whitespace()
        .filter_map(|s| {
            s.trim_matches(|c: char| c == ',' || c == ';')
                .parse::<std::net::IpAddr>()
                .ok()
                .map(|ip| ip.to_string())
        })
        .filter(|ip| server != Some(ip.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[tauri::command]
pub async fn tools_dns_hijack(domain: String) -> Result<Value, String> {
    let configs = [
        ("系统默认", None),
        ("Cloudflare", Some("1.1.1.1")),
        ("Google", Some("8.8.8.8")),
        ("AliDNS", Some("223.5.5.5")),
    ];
    let mut sources = Vec::new();
    for (name, server) in configs {
        let ips = nslookup_ips(&domain, server).await;
        sources.push(json!({"name":name,"server":server.unwrap_or("系统"),"ips":ips}));
    }
    let nonempty = sources.iter().filter(|s|s["ips"].as_array().is_some_and(|a|!a.is_empty())).count();
    let system = sources.first().and_then(|s|s["ips"].as_array()).cloned().unwrap_or_default().into_iter().filter_map(|v|v.as_str().map(str::to_string)).collect::<BTreeSet<_>>();
    let public = sources.iter().skip(1).flat_map(|s|s["ips"].as_array().cloned().unwrap_or_default()).filter_map(|v|v.as_str().map(str::to_string)).collect::<BTreeSet<_>>();
    let inconsistent = !system.is_empty() && !public.is_empty() && system.is_disjoint(&public);
    let verdict = if inconsistent { "danger" } else if nonempty < sources.len() { "warn" } else { "safe" };
    Ok(
        json!({"domain":domain,"sources":sources,"verdict":verdict,"report":match verdict{"safe"=>"系统 DNS 与多个公共解析源存在一致结果，未见明显污染。","danger"=>"系统 DNS 结果与公共解析源完全不重合，存在污染或劫持可能；CDN 域名仍需结合权威 DNS 复核。",_=>"部分解析源失败，请检查网络、域名或 DNS 可达性。"}}),
    )
}

#[tauri::command]
pub async fn tools_mtu_test(host: String) -> Result<Value, String> {
    let mut probes = Vec::new();
    let mut best = 0u32;
    for mtu in [1500u32, 1492, 1472, 1460, 1450, 1400, 1300, 1200, 1000, 576] {
        let payload = mtu.saturating_sub(28);
        let status = Command::new("ping.exe")
            .args([
                "-n",
                "1",
                "-f",
                "-l",
                &payload.to_string(),
                "-w",
                "1500",
                &host,
            ])
            .creation_flags(0x08000000)
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);
        if status && best == 0 {
            best = mtu
        }
        probes.push(json!({"mtu":mtu,"payload":payload,"ok":status}));
    }
    Ok(
        json!({"host":host,"probes":probes,"max_mtu":best,"suggestion":if best>0{format!("建议 MTU 不超过 {best}")}else{"目标不可达或禁止 ICMP".into()}}),
    )
}

#[tauri::command(async)]
pub fn tools_ssl_check(host: String) -> Result<Value, String> {
    use std::os::windows::process::CommandExt;
    let safe = host.replace('\'', "''");
    let script = format!(
        r#"$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);$tcp=[Net.Sockets.TcpClient]::new('{safe}',443);$ssl=[Net.Security.SslStream]::new($tcp.GetStream(),$false,{{$true}});$ssl.AuthenticateAsClient('{safe}');$c=[Security.Cryptography.X509Certificates.X509Certificate2]::new($ssl.RemoteCertificate);[pscustomobject]@{{ok=$true;host='{safe}';subject=$c.Subject;issuer=$c.Issuer;not_before=$c.NotBefore.ToString('s');not_after=$c.NotAfter.ToString('s');days_left=[math]::Floor(($c.NotAfter-(Get-Date)).TotalDays);thumbprint=$c.Thumbprint;protocol=$ssl.SslProtocol.ToString();cipher=$ssl.CipherAlgorithm.ToString();key_bits=$ssl.CipherStrength;san=($c.Extensions|Where-Object {{$_.Oid.FriendlyName -eq 'Subject Alternative Name'}}|ForEach-Object {{$_.Format($false)}})}}|ConvertTo-Json -Compress;$ssl.Dispose();$tcp.Dispose()"#
    );
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

async fn conn_item(label: &str, host: &str, port: u16) -> Value {
    let started = Instant::now();
    let ok = matches!(
        tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::TcpStream::connect((host, port))
        )
        .await,
        Ok(Ok(_))
    );
    json!({"label":label,"host":host,"port":port,"ok":ok,"latency_ms":if ok{Some(started.elapsed().as_secs_f64()*1000.0)}else{None},"message":if ok{"连接成功"}else{"连接失败"}})
}
#[tauri::command]
pub async fn run_chain_conn_test() -> Value {
    let items = vec![
        conn_item("DNS", "1.1.1.1", 53).await,
        conn_item("互联网", "www.baidu.com", 443).await,
        conn_item("公共服务", "223.5.5.5", 53).await,
    ];
    let success = items.iter().filter(|i| i["ok"] == true).count();
    json!({"items":items,"success_count":success,"fail_count":3-success,"avg_latency_ms":items.iter().filter_map(|i|i["latency_ms"].as_f64()).sum::<f64>()/success.max(1) as f64})
}
#[tauri::command]
pub async fn run_site_conn_test(custom_host: Option<String>, custom_port: Option<u16>) -> Value {
    let host = custom_host
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "www.baidu.com".into());
    let port = custom_port.unwrap_or(80);
    let item = conn_item("目标站点", &host, port).await;
    json!({"items":[item.clone()],"success_count":if item["ok"]==true{1}else{0},"fail_count":if item["ok"]==true{0}else{1},"avg_latency_ms":item["latency_ms"]})
}

#[tauri::command]
pub async fn protocol_analyze_snapshot() -> Result<Value, String> {
    let mut value = tools_conn_stats_detail().await?;
    value["ok"] = json!(true);
    value["message"] = json!("已分析当前 TCP/UDP 连接快照");
    Ok(value)
}

#[tauri::command]
pub async fn protocol_analyze_live(
    device: Option<String>,
    seconds: u64,
    max_packets: usize,
) -> Result<Value, String> {
    let seconds = seconds.clamp(1, 60);
    let limit = max_packets.clamp(1, 50_000);
    let mut observed = std::collections::BTreeMap::<String, Value>::new();
    for _ in 0..seconds {
        let snapshot = tools_conn_stats_detail().await?;
        for mut row in snapshot["rows"].as_array().cloned().unwrap_or_default() {
            let key = format!(
                "{}|{}|{}|{}",
                row["proto"], row["local"], row["remote"], row["pid"]
            );
            let seen = observed
                .get(&key)
                .and_then(|old| old["samples"].as_u64())
                .unwrap_or(0)
                + 1;
            row["samples"] = json!(seen);
            observed.insert(key, row);
            if observed.len() >= limit {
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let rows = observed.into_values().collect::<Vec<_>>();
    let stats = |field: &str| {
        let mut map = std::collections::BTreeMap::<String, u64>::new();
        for row in &rows {
            *map.entry(row[field].as_str().unwrap_or("Unknown").to_string())
                .or_default() += 1;
        }
        map.into_iter().collect::<Vec<_>>()
    };
    let known = rows
        .iter()
        .filter(|row| row["service"] != "Unknown")
        .count();
    Ok(
        json!({"ok":true,"message":format!("已完成 {seconds} 秒实时连接采样"),"device":device,"max_packets":limit,"total":rows.len(),"known_services":known,"known_service_count":known,"status_stats":stats("state"),"proto_stats":stats("proto"),"category_stats":stats("category"),"top_flows":rows.iter().take(20).cloned().collect::<Vec<_>>(),"rows":rows}),
    )
}
