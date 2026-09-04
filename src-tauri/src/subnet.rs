use ipnet::{Ipv4Net, Ipv6Net};
use serde_json::{json, Value};
use std::net::{Ipv4Addr, Ipv6Addr};

fn ipv4_details(net: Ipv4Net) -> Value {
    let prefix = net.prefix_len();
    let total_hosts = 1_u64 << (32 - prefix);
    let usable_hosts = if prefix <= 30 {
        total_hosts - 2
    } else {
        total_hosts
    };
    let network = net.network();
    let broadcast = net.broadcast();
    let first_host = if prefix <= 30 {
        Ipv4Addr::from(u32::from(network) + 1)
    } else {
        network
    };
    let last_host = if prefix <= 30 {
        Ipv4Addr::from(u32::from(broadcast) - 1)
    } else {
        broadcast
    };
    let mask = u32::from(net.netmask());
    let first_octet = network.octets()[0];
    let class = match first_octet {
        1..=126 => "A",
        128..=191 => "B",
        192..=223 => "C",
        224..=239 => "D",
        240..=255 => "E",
        _ => "特殊",
    };

    json!({
        "cidr": format!("{network}/{prefix}"),
        "prefix": prefix,
        "network": network.to_string(),
        "broadcast": broadcast.to_string(),
        "netmask": net.netmask().to_string(),
        "wildcard": Ipv4Addr::from(!mask).to_string(),
        "total_hosts": total_hosts,
        "usable_hosts": usable_hosts,
        "first_host": first_host.to_string(),
        "last_host": last_host.to_string(),
        "class": class,
    })
}

#[tauri::command]
pub fn calc_subnet_ipv4(cidr: String) -> Result<Value, String> {
    let net: Ipv4Net = cidr
        .parse()
        .map_err(|_| format!("无效的 IPv4 CIDR：{cidr}"))?;
    Ok(ipv4_details(net))
}

#[tauri::command]
pub fn calc_subnet_ipv6(cidr: String) -> Result<Value, String> {
    let net: Ipv6Net = cidr
        .parse()
        .map_err(|_| format!("无效的 IPv6 CIDR：{cidr}"))?;
    let prefix = net.prefix_len();
    let host_bits = 128 - prefix;
    let total_hosts = if host_bits == 128 {
        "340282366920938463463374607431768211456".to_string()
    } else {
        (1_u128 << host_bits).to_string()
    };
    let network = net.network();
    let first = u128::from(network);
    let last = if host_bits == 128 {
        u128::MAX
    } else {
        first | ((1_u128 << host_bits) - 1)
    };
    Ok(json!({
        "cidr": format!("{network}/{prefix}"),
        "network": network.to_string(),
        "prefix": prefix,
        "netmask": net.netmask().to_string(),
        "total_hosts": total_hosts,
        "first_host": Ipv6Addr::from(first).to_string(),
        "last_host": Ipv6Addr::from(last).to_string(),
        "compressed": network.to_string(),
        "expanded": network.segments().iter().map(|part| format!("{part:04x}")).collect::<Vec<_>>().join(":"),
    }))
}

#[tauri::command]
pub fn calc_wildcard_mask(
    ip: String,
    wildcard: String,
) -> Result<(String, String, String), String> {
    let address: Ipv4Addr = ip.parse().map_err(|_| "无效的 IPv4 地址".to_string())?;
    let wildcard_addr: Ipv4Addr = wildcard.parse().map_err(|_| "无效的反掩码".to_string())?;
    let address_u32 = u32::from(address);
    let wildcard_u32 = u32::from(wildcard_addr);
    let start = Ipv4Addr::from(address_u32 & !wildcard_u32);
    let end = Ipv4Addr::from(address_u32 | wildcard_u32);
    Ok((
        start.to_string(),
        end.to_string(),
        Ipv4Addr::from(!wildcard_u32).to_string(),
    ))
}

#[tauri::command]
pub fn split_subnets(
    base: String,
    new_prefix: u8,
    limit: Option<usize>,
) -> Result<Vec<Value>, String> {
    let net: Ipv4Net = base
        .parse()
        .map_err(|_| format!("无效的 IPv4 CIDR：{base}"))?;
    if new_prefix < net.prefix_len() || new_prefix > 32 {
        return Err("新前缀必须大于等于原前缀且不超过 32".into());
    }
    let count = 1_u64 << (new_prefix - net.prefix_len());
    let cap = limit.unwrap_or(256).min(count as usize);
    let block = 1_u32.checked_shl((32 - new_prefix) as u32).unwrap_or(0);
    let base_u32 = u32::from(net.network());
    let mut rows = Vec::with_capacity(cap);
    for index in 0..cap {
        let network = Ipv4Addr::from(base_u32.wrapping_add(block.wrapping_mul(index as u32)));
        let child = Ipv4Net::new(network, new_prefix).map_err(|error| error.to_string())?;
        let details = ipv4_details(child);
        rows.push(json!({
            "index": index + 1,
            "cidr": details["cidr"],
            "network": details["network"],
            "broadcast": details["broadcast"],
            "first_host": details["first_host"],
            "last_host": details["last_host"],
        }));
    }
    Ok(rows)
}

#[tauri::command]
pub fn summarize_routes(cidrs: Vec<String>) -> Result<Value, String> {
    if cidrs.is_empty() {
        return Err("请输入至少一个网段".into());
    }
    let mut nets = cidrs
        .iter()
        .map(|cidr| {
            cidr.parse::<Ipv4Net>()
                .map_err(|_| format!("无效的 CIDR：{cidr}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    nets.sort_by_key(|net| u32::from(net.network()));
    let start = u32::from(nets.first().unwrap().network());
    let end = nets
        .iter()
        .map(|net| u32::from(net.broadcast()))
        .max()
        .unwrap();
    let xor = start ^ end;
    let prefix = xor.leading_zeros() as u8;
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    let summary_net =
        Ipv4Net::new(Ipv4Addr::from(start & mask), prefix).map_err(|error| error.to_string())?;
    let covered: u64 = nets
        .iter()
        .map(|net| 1_u64 << (32 - net.prefix_len()))
        .sum();
    Ok(json!({
        "message": "汇总完成",
        "summary": summary_net.to_string(),
        "prefix": prefix,
        "count": nets.len(),
        "covered": covered,
    }))
}

#[tauri::command]
pub fn calc_vlsm(base: String, mut requirements: Vec<(String, u64)>) -> Result<Vec<Value>, String> {
    let net: Ipv4Net = base
        .parse()
        .map_err(|_| format!("无效的 IPv4 CIDR：{base}"))?;
    requirements.sort_by(|a, b| b.1.cmp(&a.1));
    let mut cursor = u32::from(net.network()) as u64;
    let end = u32::from(net.broadcast()) as u64;
    let mut rows = Vec::new();
    for (name, needed) in requirements {
        let allocated = (needed.saturating_add(2)).next_power_of_two().max(2);
        let prefix = 32 - allocated.trailing_zeros() as u8;
        cursor = (cursor + allocated - 1) & !(allocated - 1);
        if cursor + allocated - 1 > end {
            return Err(format!("地址空间不足，无法分配：{name}"));
        }
        let child = Ipv4Net::new(Ipv4Addr::from(cursor as u32), prefix)
            .map_err(|error| error.to_string())?;
        let details = ipv4_details(child);
        rows.push(json!({
            "name": name,
            "needed": needed,
            "allocated": allocated,
            "cidr": details["cidr"],
            "first_host": details["first_host"],
            "last_host": details["last_host"],
        }));
        cursor += allocated;
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_calculation_matches_standard_network_math() {
        let value = calc_subnet_ipv4("192.168.10.37/24".into()).unwrap();
        assert_eq!(value["network"], "192.168.10.0");
        assert_eq!(value["broadcast"], "192.168.10.255");
        assert_eq!(value["usable_hosts"], 254);
        assert_eq!(value["wildcard"], "0.0.0.255");
    }

    #[test]
    fn split_subnets_returns_expected_blocks() {
        let rows = split_subnets("10.0.0.0/24".into(), 26, Some(256)).unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0]["cidr"], "10.0.0.0/26");
        assert_eq!(rows[3]["cidr"], "10.0.0.192/26");
    }

    #[test]
    fn vlsm_places_largest_requirement_first() {
        let rows = calc_vlsm(
            "10.0.0.0/24".into(),
            vec![("small".into(), 10), ("large".into(), 100)],
        )
        .unwrap();
        assert_eq!(rows[0]["name"], "large");
        assert_eq!(rows[0]["cidr"], "10.0.0.0/25");
        assert_eq!(rows[1]["cidr"], "10.0.0.128/28");
    }
}
