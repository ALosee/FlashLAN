use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::Duration};

pub const SERVICE_TYPE: &str = "_flashlan._tcp.local.";
pub const SERVICE_PORT: u16 = 17321;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub platform: String,
    pub port: u16,
}

fn sanitize_hostname(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
}

fn get_hostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown".to_string())
}

fn get_local_ip() -> String {
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

pub fn get_device_info() -> DeviceInfo {
    let hostname = get_hostname();
    DeviceInfo {
        id: format!("{}-{}", hostname, get_local_ip()),
        name: hostname,
        ip: get_local_ip(),
        platform: std::env::consts::OS.to_string(),
        port: SERVICE_PORT,
    }
}

/// Register mDNS service for this device. Keeps daemon alive via forget or state.
pub fn register_mdns() -> Result<ServiceDaemon, String> {
    let daemon = ServiceDaemon::new().map_err(|e| format!("mdns daemon: {e}"))?;
    let hostname = get_hostname();
    let ip = get_local_ip();
    let host_ipv4: std::net::IpAddr = ip.parse().unwrap_or_else(|_| "127.0.0.1".parse().unwrap());
    let instance_name = format!("{}-{}", sanitize_hostname(&hostname), &ip.replace('.', "-"));
    let host_name = format!("{}.local.", sanitize_hostname(&hostname));
    let properties = [
        ("platform".to_string(), std::env::consts::OS.to_string()),
        ("id".to_string(), format!("{}-{}", hostname, ip)),
    ];
    // Collect properties as slice of (&str, &str) for ServiceInfo
    let props_ref: Vec<(&str, &str)> = properties.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let service_info = ServiceInfo::new(
        SERVICE_TYPE,
        &instance_name,
        &host_name,
        host_ipv4,
        SERVICE_PORT,
        &props_ref[..],
    )
    .map_err(|e| format!("service info: {e}"))?;
    daemon.register(service_info).map_err(|e| format!("register: {e}"))?;
    Ok(daemon)
}

pub async fn discover_devices(timeout_ms: u64) -> Vec<DeviceInfo> {
    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    let Ok(receiver) = daemon.browse(SERVICE_TYPE) else {
        return vec![];
    };

    let timeout = Duration::from_millis(timeout_ms);
    let start = std::time::Instant::now();
    let mut seen = HashSet::new();
    let mut devices = Vec::new();
    let local_ip = get_local_ip();
    let local_hostname = get_hostname();

    while start.elapsed() < timeout {
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() { break; }
        let recv_timeout = std::cmp::min(remaining, Duration::from_millis(300));
        match receiver.recv_timeout(recv_timeout) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let fullname = info.get_fullname().to_string();
                if seen.contains(&fullname) { continue; }
                seen.insert(fullname.clone());
                let hostname = info.get_hostname().trim_end_matches('.').to_string();
                // Filter self by hostname+ip
                let addrs = info.get_addresses();
                let ip = addrs.iter().next().map(|a| a.to_string()).unwrap_or_default();
                if ip == local_ip && hostname.contains(&sanitize_hostname(&local_hostname)) {
                    continue;
                }
                if ip.is_empty() || ip == "127.0.0.1" { continue; }
                let props = info.get_properties();
                let platform = props.get("platform").map(|v| v.val_str().to_string()).unwrap_or_else(|| "unknown".to_string());
                let id = props.get("id").map(|v| v.val_str().to_string()).unwrap_or_else(|| fullname.clone());
                let name = hostname.split('.').next().unwrap_or(&hostname).to_string();
                // Fallback to instance name if hostname empty
                let port = info.get_port();
                devices.push(DeviceInfo { id, name, ip, platform, port });
            }
            Ok(_) => continue,
            Err(_) => continue,
        }
    }
    // daemon will be dropped here, which unregisters browse but not service registrations of other daemon
    devices
}
