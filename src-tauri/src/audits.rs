use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::process::Command;
use tauri::{AppHandle, Emitter};

fn ps_json(script: &str) -> Result<Value, String> {
    let wrapped=format!("$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);{script}");
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &wrapped])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_health_check(app: AppHandle) -> Result<bool, String> {
    let interfaces = crate::interfaces::all_interfaces()?;
    let gateway = interfaces
        .iter()
        .find(|i| i["is_lan_primary"] == true)
        .and_then(|i| i["gateway"].as_str())
        .map(str::to_string);
    let network_ok = interfaces.iter().any(|item| item["is_up"] == true);
    let gateway_started = std::time::Instant::now();
    let gateway_ok = if let Some(address) = &gateway {
        Command::new("ping.exe")
            .args(["-n", "1", "-w", "1500", address])
            .creation_flags(0x08000000)
            .output()
            .is_ok_and(|out| out.status.success())
    } else {
        false
    };
    let gateway_ms = gateway_ok.then(|| gateway_started.elapsed().as_millis());
    let dns_started = std::time::Instant::now();
    let dns_ok = tokio::net::lookup_host(("www.baidu.com", 80)).await.is_ok();
    let dns_ms = dns_ok.then(|| dns_started.elapsed().as_millis());
    let internet_started = std::time::Instant::now();
    let internet_ok = matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            tokio::net::TcpStream::connect(("www.baidu.com", 443))
        )
        .await,
        Ok(Ok(_))
    );
    let internet_ms = internet_ok.then(|| internet_started.elapsed().as_millis());
    let latency_ok = internet_ms.is_some_and(|ms| ms <= 250);
    let link_speed = interfaces
        .iter()
        .filter_map(|item| item["speed"].as_u64())
        .max()
        .unwrap_or(0);
    let speed_ok = link_speed > 0;
    let adapter_stats = ps_json(r#"$e=[uint64]0;$d=[uint64]0;Get-NetAdapterStatistics|ForEach-Object{$e += [uint64]$_.ReceivedPacketErrors+[uint64]$_.OutboundPacketErrors;$d += [uint64]$_.ReceivedDiscardedPackets+[uint64]$_.OutboundDiscardedPackets};[pscustomobject]@{errors=$e;discards=$d}|ConvertTo-Json -Compress"#).unwrap_or_else(|_|json!({"errors":0,"discards":0}));
    let loop_errors = adapter_stats["errors"].as_u64().unwrap_or(0)
        + adapter_stats["discards"].as_u64().unwrap_or(0);
    let checks = vec![
        (
            "network_card",
            "网卡状态",
            10,
            network_ok,
            format!("检测到 {} 个 IPv4 接口", interfaces.len()),
            "检查网卡是否启用并确认驱动状态",
        ),
        (
            "gateway",
            "网关连通",
            20,
            gateway_ok,
            format!(
                "{} · {}",
                gateway.clone().unwrap_or_else(|| "未配置".into()),
                gateway_ms
                    .map(|v| format!("{v} ms"))
                    .unwrap_or_else(|| "不可达".into())
            ),
            "检查默认网关、VLAN 与物理链路",
        ),
        (
            "dns",
            "DNS 解析",
            30,
            dns_ok,
            format!(
                "www.baidu.com · {}",
                dns_ms
                    .map(|v| format!("{v} ms"))
                    .unwrap_or_else(|| "失败".into())
            ),
            "检查 DNS 地址或尝试更换公共 DNS",
        ),
        (
            "internet",
            "外网连接",
            20,
            internet_ok,
            format!(
                "www.baidu.com:443 · {}",
                internet_ms
                    .map(|v| format!("{v} ms"))
                    .unwrap_or_else(|| "失败".into())
            ),
            "检查上游出口、防火墙和代理设置",
        ),
        (
            "latency",
            "网络延迟",
            10,
            latency_ok,
            internet_ms
                .map(|v| format!("HTTPS 建连 {v} ms"))
                .unwrap_or_else(|| "无有效样本".into()),
            "网络延迟偏高，检查链路拥塞与无线信号",
        ),
        (
            "speed",
            "网络速度",
            5,
            speed_ok,
            if link_speed > 0 {
                format!("网卡协商速率 {:.1} Mbps", link_speed as f64 / 1_000_000.0)
            } else {
                "未取得协商速率".into()
            },
            "检查网线规格、双工和网卡协商速率",
        ),
        (
            "loop_detection",
            "环路检测",
            5,
            loop_errors == 0,
            format!("错误/丢弃计数 {loop_errors}"),
            "检查交换机 STP、广播风暴与重复接线",
        ),
    ];
    let mut items = Vec::new();
    for (index, (id, title, max_score, ok, details, suggestion)) in checks.into_iter().enumerate() {
        let item = json!({"id":id,"title":title,"status":if ok{"正常"}else{"异常"},"value":if ok{"正常"}else{"异常"},"details":details,"detail":details,"score":if ok{max_score}else{0},"max_score":max_score,"suggestion":if ok{"无需处理"}else{suggestion}});
        items.push(item.clone());
        let _ = app.emit("health:item", item);
        let _ = app.emit(
            "health:progress",
            json!({"progress":((index+1)*100/7),"message":format!("正在检查：{title}")}),
        );
    }
    let total_score = items
        .iter()
        .filter_map(|i| i["score"].as_u64())
        .sum::<u64>();
    let grade = if total_score >= 90 {
        "A"
    } else if total_score >= 75 {
        "B"
    } else if total_score >= 60 {
        "C"
    } else {
        "D"
    };
    let result = json!({"total_score":total_score,"grade":grade,"items":items,"suggestions":items.iter().filter(|i|i["status"]!="ok").map(|i|i["suggestion"].clone()).collect::<Vec<_>>()});
    let _ = app.emit("health:complete", result);
    Ok(true)
}

#[tauri::command(async)]
pub fn start_loop_detection(app: AppHandle, iface_key: Option<String>) -> Result<Value, String> {
    let selected_name = if let Some(key) = iface_key.as_deref().filter(|key|*key!="__all__") {
        crate::interfaces::all_interfaces()?.into_iter().find(|item|item["key"]==key).and_then(|item|item["name"].as_str().map(str::to_string))
    } else { None };
    let stats = ps_json(
        r#"@((Get-NetAdapterStatistics)|ForEach-Object{[pscustomobject]@{name=$_.Name;in_discards=[uint64]$_.ReceivedDiscardedPackets;out_discards=[uint64]$_.OutboundDiscardedPackets;in_errors=[uint64]$_.ReceivedPacketErrors;out_errors=[uint64]$_.OutboundPacketErrors}})|ConvertTo-Json -Compress"#,
    )?;
    let mut rows = stats.as_array().cloned().unwrap_or_default();
    if let Some(name) = selected_name {
        rows.retain(|r| r["name"] == name);
    }
    let errors = rows
        .iter()
        .map(|r| {
            r["in_discards"].as_u64().unwrap_or(0)
                + r["out_discards"].as_u64().unwrap_or(0)
                + r["in_errors"].as_u64().unwrap_or(0)
                + r["out_errors"].as_u64().unwrap_or(0)
        })
        .sum::<u64>();
    let risk = if errors > 1000 {
        "高"
    } else if errors > 0 {
        "注意"
    } else {
        "低"
    };
    let result = json!({"status":if errors==0{"正常"}else{"发现丢包/错误计数"},"risk_level":risk,"total_score":if errors==0{100}else{60},"parts":rows,"suggestion":if errors==0{"未发现明显环路迹象"}else{"检查交换机 STP、广播流量和物理连线"}});
    let _ = app.emit(
        "loop:progress",
        json!({"progress":100,"message":"检测完成"}),
    );
    let _ = app.emit("loop:complete", &result);
    Ok(result)
}

#[tauri::command(async)]
pub fn start_security_test(app: AppHandle) -> Result<bool, String> {
    let raw = ps_json(
        r#"$fw=@(Get-NetFirewallProfile|Where-Object{$_.Enabled}).Count;$rdp=(Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -ErrorAction SilentlyContinue).fDenyTSConnections;$smb=(Get-WindowsOptionalFeature -Online -FeatureName SMB1Protocol -ErrorAction SilentlyContinue).State;$guest=(Get-LocalUser -Name Guest -ErrorAction SilentlyContinue).Enabled;$uac=(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' -Name EnableLUA -ErrorAction SilentlyContinue).EnableLUA;$policy=[ADSI]("WinNT://"+$env:COMPUTERNAME);$screen=Get-ItemProperty 'HKCU:\Control Panel\Desktop' -ErrorAction SilentlyContinue;$au=(Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -ErrorAction SilentlyContinue).NoAutoUpdate;$os=Get-CimInstance Win32_OperatingSystem;$shares=@(Get-SmbShare -ErrorAction SilentlyContinue|Where-Object{$_.Special -ne $true});$ports=@(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue|Where-Object LocalPort -in @(21,23,135,139,445,3389,5900)|Select-Object -ExpandProperty LocalPort -Unique);[pscustomobject]@{firewall=($fw -eq 3);rdp_disabled=($rdp -eq 1);smb1_disabled=($smb -ne 'Enabled');guest_disabled=($guest -ne $true);uac=($uac -eq 1);pwd_length=[int]$policy.MinPasswordLength.Value;lockout=[int]$policy.MaxBadPasswordsAllowed.Value;screensaver=([string]$screen.ScreenSaveActive -eq '1' -and [string]$screen.ScreenSaverIsSecure -eq '1');screen_timeout=[int]$screen.ScreenSaveTimeOut;autoupdate=($au -ne 1);os=($os.Caption+' '+$os.Version+' Build '+$os.BuildNumber);shares=@($shares|Select-Object -ExpandProperty Name);ports=$ports}|ConvertTo-Json -Depth 4 -Compress"#,
    )?;
    let pwd = raw["pwd_length"].as_i64().unwrap_or(0);
    let lockout = raw["lockout"].as_i64().unwrap_or(0);
    let screen_timeout = raw["screen_timeout"].as_i64().unwrap_or(0);
    let shares = raw["shares"].as_array().cloned().unwrap_or_default();
    let ports = raw["ports"].as_array().cloned().unwrap_or_default();
    let specs = vec![
        (
            "firewall",
            "Windows 防火墙",
            "账户与访问控制",
            12,
            raw["firewall"] == true,
            format!(
                "三个网络配置文件{}启用",
                if raw["firewall"] == true {
                    "均已"
                } else {
                    "未全部"
                }
            ),
            "请启用域、专用和公用防火墙",
        ),
        (
            "smb1",
            "SMBv1 危险协议",
            "协议安全",
            12,
            raw["smb1_disabled"] == true,
            if raw["smb1_disabled"] == true {
                "SMBv1 协议已禁用或未配置（默认安全）".into()
            } else {
                "SMBv1 协议已启用（存在 EternalBlue/WannaCry 风险）".into()
            },
            "执行 Set-SmbServerConfiguration -EnableSMB1Protocol $false",
        ),
        (
            "uac",
            "UAC 用户账户控制",
            "账户与访问控制",
            10,
            raw["uac"] == true,
            if raw["uac"] == true {
                "UAC 已启用".into()
            } else {
                "UAC 已关闭".into()
            },
            "启用 UAC 并保持默认通知级别",
        ),
        (
            "guest",
            "Guest 账户状态",
            "账户与访问控制",
            8,
            raw["guest_disabled"] == true,
            if raw["guest_disabled"] == true {
                "Guest 账户已禁用".into()
            } else {
                "Guest（来宾）账户已启用".into()
            },
            "执行 net user guest /active:no",
        ),
        (
            "rdp",
            "远程桌面 (RDP)",
            "网络暴露面",
            8,
            raw["rdp_disabled"] == true,
            if raw["rdp_disabled"] == true {
                "远程桌面未启用".into()
            } else {
                "远程桌面已启用".into()
            },
            "不使用时关闭 RDP；需要时启用 NLA 并限制来源",
        ),
        (
            "pwd_length",
            "密码最小长度",
            "密码与锁定策略",
            10,
            pwd >= 8,
            format!("当前最小密码长度 {pwd}"),
            "建议最小密码长度至少 8 位",
        ),
        (
            "lockout",
            "账户锁定策略",
            "密码与锁定策略",
            10,
            lockout > 0 && lockout <= 10,
            format!("当前锁定阈值 {lockout} 次"),
            "建议配置 3–10 次失败后锁定",
        ),
        (
            "screensaver",
            "屏幕自动锁定",
            "账户与访问控制",
            8,
            raw["screensaver"] == true && screen_timeout > 0 && screen_timeout <= 600,
            format!(
                "安全屏保={}，超时={} 秒",
                raw["screensaver"], screen_timeout
            ),
            "启用恢复时登录并将等待时间设为 10 分钟以内",
        ),
        (
            "autoupdate",
            "Windows 自动更新",
            "系统更新与补丁",
            8,
            raw["autoupdate"] == true,
            if raw["autoupdate"] == true {
                "未检测到禁用自动更新策略".into()
            } else {
                "自动更新已被策略关闭".into()
            },
            "启用 Windows Update 并及时安装安全更新",
        ),
        (
            "os_version",
            "操作系统版本",
            "系统更新与补丁",
            6,
            true,
            raw["os"].as_str().unwrap_or("Windows").to_string(),
            "保持操作系统在支持周期内",
        ),
        (
            "shares",
            "本地共享目录",
            "网络暴露面",
            4,
            shares.is_empty(),
            format!(
                "非管理共享：{}",
                if shares.is_empty() {
                    "无".into()
                } else {
                    shares
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
            "删除非必要共享并收紧共享权限",
        ),
        (
            "ports",
            "高危端口开放",
            "网络暴露面",
            4,
            ports.is_empty(),
            format!(
                "监听端口：{}",
                if ports.is_empty() {
                    "无".into()
                } else {
                    ports
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
            "关闭非必要的高风险监听端口或限制防火墙来源",
        ),
    ];
    let mut items = Vec::new();
    for (idx, (id, title, category, max_score, safe, detail, suggestion)) in
        specs.into_iter().enumerate()
    {
        let status = if safe {
            "safe"
        } else if matches!(id, "smb1" | "guest" | "uac") {
            "danger"
        } else {
            "warning"
        };
        let item = json!({"id":id,"title":title,"category":category,"status":status,"detail":detail,"suggestion":if safe{"无需处理"}else{suggestion},"score":if safe{max_score}else{0},"max_score":max_score});
        items.push(item.clone());
        let _ = app.emit("security:item", item);
        let _ = app.emit(
            "security:progress",
            json!({"progress":((idx+1)*100/12),"message":format!("检查：{title}")}),
        );
    }
    let score = items
        .iter()
        .filter_map(|i| i["score"].as_u64())
        .sum::<u64>();
    let warning = items.iter().filter(|i| i["status"] == "warning").count();
    let danger = items.iter().filter(|i| i["status"] == "danger").count();
    let safe = items.iter().filter(|i| i["status"] == "safe").count();
    let info = items.iter().filter(|i| i["status"] == "info").count();
    let result = json!({"score":score,"grade":if score>=90{"A"}else if score>=75{"B"}else if score>=60{"C"}else{"D"},"safe":safe,"warning":warning,"danger":danger,"info":info,"items":items,"summary":format!("完成 {} 项安全检查",items.len()),"report_text":format!("网络测试工具箱安全自测\r\n得分：{score}\r\n高危：{danger} 项\r\n中危：{warning} 项")});
    let _ = app.emit("security:complete", result);
    Ok(true)
}

#[tauri::command(async)]
pub fn start_log_audit(app: AppHandle, hours: u32, max_events: u32) -> Result<bool, String> {
    let script = format!(
        r#"$start=(Get-Date).AddHours(-{});$events=@(Get-WinEvent -FilterHashtable @{{LogName='Security';StartTime=$start}} -MaxEvents {} -ErrorAction Stop);$names=@{{4624='登录成功';4625='登录失败';4634='注销';4648='显式凭据登录';4672='特殊权限登录';4688='进程创建';4719='审核策略变更';4720='创建用户';4726='删除用户';4732='加入本地组';1102='清除审核日志'}};$danger=@(1102,4720,4726,4732);$warning=@(4625,4648,4672,4719);$counts=@($events|Group-Object Id|Sort-Object Count -Descending|Select-Object -First 20|ForEach-Object{{$id=[int]$_.Name;[pscustomobject]@{{id=$id;name=if($names[$id]){{$names[$id]}}else{{'事件 '+$id}};count=[int]$_.Count;severity=if($danger -contains $id){{'danger'}}elseif($warning -contains $id){{'warning'}}else{{'safe'}}}}}});$ips=@($events|Where-Object Id -eq 4625|ForEach-Object{{try{{$xml=[xml]$_.ToXml();($xml.Event.EventData.Data|Where-Object Name -eq 'IpAddress').'#text'}}catch{{}}}}|Where-Object{{$_ -and $_ -notin @('-','127.0.0.1','::1')}}|Group-Object|Sort-Object Count -Descending|Select-Object -First 20|ForEach-Object{{[pscustomobject]@{{name=$_.Name;count=[int]$_.Count}}}});[pscustomobject]@{{total=$events.Count;success_logins=@($events|Where-Object Id -eq 4624).Count;fail_logins=@($events|Where-Object Id -eq 4625).Count;event_counts=$counts;fail_ips=$ips}}|ConvertTo-Json -Depth 5 -Compress"#,
        hours.clamp(1, 720),
        max_events.clamp(10, 20_000)
    );
    let raw = ps_json(&script)?;
    let counts = raw["event_counts"].as_array().cloned().unwrap_or_default();
    let risks = counts.iter().filter(|item|item["severity"]=="danger"||item["severity"]=="warning").map(|item|json!({"severity":item["severity"],"title":item["name"],"detail":format!("事件 ID {} 在所选时段出现 {} 次",item["id"],item["count"])})).collect::<Vec<_>>();
    let risk_score = counts
        .iter()
        .map(|item| match item["severity"].as_str() {
            Some("danger") => item["count"].as_u64().unwrap_or(0) * 5,
            Some("warning") => item["count"].as_u64().unwrap_or(0),
            _ => 0,
        })
        .sum::<u64>();
    let result = json!({"ok":true,"time_span":format!("最近 {hours} 小时"),"total":raw["total"],"success_logins":raw["success_logins"],"fail_logins":raw["fail_logins"],"risk_score":risk_score,"event_counts":counts,"fail_ips":raw["fail_ips"],"risks":risks,"error":null,"report_text":format!("Windows 安全日志审计\r\n范围：最近 {hours} 小时\r\n事件总数：{}\r\n登录成功：{}\r\n登录失败：{}\r\n风险分：{risk_score}",raw["total"],raw["success_logins"],raw["fail_logins"])});
    let _ = app.emit(
        "logaudit:progress",
        json!({"progress":100,"message":"读取完成"}),
    );
    let _ = app.emit("logaudit:complete", result);
    Ok(true)
}
