use serde_json::{json, Value};
use std::collections::BTreeMap;
use tokio::process::Command;

async fn route_command(args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("route.exe")
        .args(args)
        .creation_flags(0x08000000)
        .output()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_route_table() -> Result<Value, String> {
    let output = route_command(&["print", "-4"]).await?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut routes = Vec::new();
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() == 5
            && fields[0].parse::<std::net::Ipv4Addr>().is_ok()
            && fields[1].parse::<std::net::Ipv4Addr>().is_ok()
            && fields[4].parse::<u32>().is_ok()
        {
            routes.push(
                json!({"network": fields[0], "mask": fields[1], "gateway": fields[2],
                "interface": fields[3], "metric": fields[4].parse::<u32>().unwrap_or(0)}),
            );
        }
    }
    let default_count = routes
        .iter()
        .filter(|row| row["network"] == "0.0.0.0")
        .count();
    let onlink_count = routes
        .iter()
        .filter(|row| {
            row["gateway"].as_str().is_some_and(|value| {
                value.eq_ignore_ascii_case("On-link") || value.contains("链路")
            })
        })
        .count();
    let remote_count = routes.len().saturating_sub(default_count + onlink_count);
    let mut iface_stats = BTreeMap::<String, usize>::new();
    let mut metric_buckets = BTreeMap::<String, usize>::new();
    for row in &routes {
        *iface_stats
            .entry(row["interface"].as_str().unwrap_or("未知").to_string())
            .or_default() += 1;
        let metric = row["metric"].as_u64().unwrap_or(0);
        let bucket = match metric {
            0..=20 => "0-20",
            21..=50 => "21-50",
            51..=100 => "51-100",
            _ => ">100",
        };
        *metric_buckets.entry(bucket.into()).or_default() += 1;
    }
    let iface_stats = iface_stats.into_iter().collect::<Vec<_>>();
    let metric_buckets = metric_buckets.into_iter().collect::<Vec<_>>();
    Ok(
        json!({"routes": routes, "default_count": default_count, "onlink_count": onlink_count,
        "remote_count": remote_count, "iface_stats": iface_stats, "metric_buckets": metric_buckets}),
    )
}

#[tauri::command]
pub async fn add_route(dest: String, mask: String, gateway: String) -> Result<String, String> {
    let output = route_command(&["add", &dest, "mask", &mask, &gateway]).await?;
    if output.status.success() {
        Ok("路由添加成功".into())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

#[tauri::command]
pub async fn delete_route(dest: String) -> Result<String, String> {
    let output = route_command(&["delete", &dest]).await?;
    if output.status.success() {
        Ok("路由删除成功".into())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}
