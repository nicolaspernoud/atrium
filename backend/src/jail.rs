use crate::OptionalJail;
use crate::configuration::JailConfig;
use dashmap::{DashMap, DashSet};
use iptables::IPTables;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

pub struct Jail {
    config: JailConfig,
    failures: DashMap<IpAddr, Vec<Instant>>,
    banned_ips: DashSet<IpAddr>,
    ipt: Arc<IPTables>,
    ipt6: Arc<IPTables>,
}

impl Jail {
    pub async fn new_from_config(jail_config: &JailConfig) -> OptionalJail {
        let mut jail = None;
        if jail_config.enabled {
            let ipt = iptables::new(false).ok();
            let ipt6 = iptables::new(true).ok();
            if let (Some(ipt), Some(ipt6)) = (ipt, ipt6) {
                let ipt = Arc::new(ipt);
                let ipt6 = Arc::new(ipt6);
                if Self::init_iptables(Arc::clone(&ipt)).await.is_ok()
                    && Self::init_iptables(Arc::clone(&ipt6)).await.is_ok()
                {
                    let jail_obj = Self {
                        config: jail_config.clone(),
                        failures: DashMap::new(),
                        banned_ips: DashSet::new(),
                        ipt,
                        ipt6,
                    };
                    jail_obj.load_banned_ips().await;
                    jail = Some(std::sync::Arc::new(jail_obj));
                }
            }
        }
        jail
    }

    pub async fn init_iptables(
        ipt: Arc<IPTables>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tokio::task::spawn_blocking(move || {
            if !ipt.chain_exists("filter", "ATRIUM_JAIL").unwrap_or(false) {
                ipt.new_chain("filter", "ATRIUM_JAIL")
                    .map_err(|e| e.to_string())?;
                info!("Created {} chain ATRIUM_JAIL", ipt.cmd);
            }
            if !ipt
                .exists("filter", "INPUT", "-j ATRIUM_JAIL")
                .unwrap_or(false)
            {
                ipt.insert("filter", "INPUT", "-j ATRIUM_JAIL", 1)
                    .map_err(|e| e.to_string())?;
                info!("Linked {} chain ATRIUM_JAIL to INPUT", ipt.cmd);
            }
            Ok(())
        })
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        .map_err(|e: String| e.into())
    }

    async fn load_banned_ips(&self) {
        let ipt = Arc::clone(&self.ipt);
        let ipt6 = Arc::clone(&self.ipt6);

        let ips = tokio::task::spawn_blocking(move || {
            let mut ips = Vec::new();
            for ipt_tool in [&ipt, &ipt6] {
                if let Ok(rules) = ipt_tool.list("filter", "ATRIUM_JAIL") {
                    for rule in rules {
                        if let Some(ip) = Self::extract_ip(&rule) {
                            ips.push(ip);
                        }
                    }
                }
            }
            ips
        })
        .await
        .unwrap_or_default();

        for ip in ips {
            self.banned_ips.insert(ip);
        }
        info!("Loaded {} banned IPs from iptables", self.banned_ips.len());
    }

    pub fn extract_ip(rule: &str) -> Option<IpAddr> {
        let mut it = rule.split_whitespace().peekable();
        while let Some(token) = it.next() {
            if token == "-s" {
                let ip_str = it.next()?;
                let ip_only = ip_str.split_once('/').map_or(ip_str, |(a, _)| a);
                return ip_only.parse().ok();
            }
        }
        None
    }

    pub async fn report_failure(&self, ip: IpAddr) {
        let ip = Self::normalize_ip(ip);

        if ip.is_loopback()
            || self.config.whitelist.contains(&ip)
            || matches!(ip, IpAddr::V4(v4) if v4.is_private())
        {
            return;
        }

        let now = Instant::now();
        let mut entry = self.failures.entry(ip).or_default();

        if Self::update_failures(
            entry.value_mut(),
            now,
            Duration::from_secs(self.config.find_time),
            self.config.max_retry,
        ) {
            // avoid deadlock
            drop(entry);
            self.ban_ip(ip).await;
            self.failures.remove(&ip);
        }
    }

    #[inline]
    fn normalize_ip(ip: IpAddr) -> IpAddr {
        match ip {
            IpAddr::V6(v6) => {
                if let Some(v4) = v6.to_ipv4_mapped() {
                    IpAddr::V4(v4)
                } else {
                    IpAddr::V6(v6)
                }
            }
            IpAddr::V4(_) => ip,
        }
    }

    fn update_failures(
        entry: &mut Vec<Instant>,
        now: Instant,
        find_time: Duration,
        max_retry: u32,
    ) -> bool {
        entry.push(now);
        entry.retain(|&t| now.duration_since(t) <= find_time);
        entry.len() >= max_retry as usize
    }

    async fn ban_ip(&self, ip: IpAddr) {
        if self.banned_ips.contains(&ip) {
            return;
        }
        self.banned_ips.insert(ip);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let (rule, is_ipv6) = Self::generate_ban_rule(ip, timestamp);
        let ipt = if is_ipv6 {
            Arc::clone(&self.ipt6)
        } else {
            Arc::clone(&self.ipt)
        };

        let res = tokio::task::spawn_blocking(move || {
            ipt.append("filter", "ATRIUM_JAIL", &rule)
                .map_err(|e| e.to_string())
        })
        .await;

        match res {
            Ok(Err(e)) => warn!("Failed to ban IP {}: {}", ip, e),
            Ok(Ok(_)) => info!("BANNED IP: {}", ip),
            Err(e) => warn!("Task join error during ban IP {}: {}", ip, e),
        }
    }

    #[inline]
    fn generate_ban_rule(ip: IpAddr, timestamp: u64) -> (String, bool) {
        let is_ipv6 = matches!(ip, IpAddr::V6(_));
        let rule = format!("-s {} -j DROP -m comment --comment {}", ip, timestamp);
        (rule, is_ipv6)
    }

    pub async fn prune_expired_rules(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let ban_duration = self.config.ban_time * 24 * 3600;

        let ipt = Arc::clone(&self.ipt);
        let ipt6 = Arc::clone(&self.ipt6);

        let unbanned_ips = tokio::task::spawn_blocking(move || {
            let mut unbanned = Vec::new();
            for ipt_tool in [&ipt, &ipt6] {
                if let Ok(rules) = ipt_tool.list("filter", "ATRIUM_JAIL") {
                    for rule in rules {
                        if let Some(timestamp) = Self::extract_timestamp(&rule)
                            && now > timestamp + ban_duration
                            && let Some(rule_content) = rule.strip_prefix("-A ATRIUM_JAIL ")
                        {
                            if let Err(e) = ipt_tool.delete("filter", "ATRIUM_JAIL", rule_content) {
                                warn!("Failed to delete expired rule {}: {}", rule, e);
                            } else if let Some(ip) = Self::extract_ip(&rule) {
                                unbanned.push(ip);
                            }
                        }
                    }
                }
            }
            unbanned
        })
        .await
        .unwrap_or_default();

        for ip in unbanned_ips {
            self.banned_ips.remove(&ip);
            info!("UNBANNED IP: {}", ip);
        }
    }

    pub(crate) fn extract_timestamp(rule: &str) -> Option<u64> {
        if let Some(pos) = rule.find("--comment ") {
            let timestamp_str = &rule[pos + 10..];
            let timestamp_str = timestamp_str.split_whitespace().next()?;
            let timestamp_str = timestamp_str.trim_matches('\"');
            timestamp_str.parse::<u64>().ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_extract_ip() {
        assert_eq!(
            Jail::extract_ip("-A ATRIUM_JAIL -s 1.2.3.4/32 -j DROP"),
            Some("1.2.3.4".parse().unwrap())
        );
        assert_eq!(
            Jail::extract_ip("-s 192.168.1.1 -j ACCEPT"),
            Some("192.168.1.1".parse().unwrap())
        );
        assert_eq!(
            Jail::extract_ip("-A ATRIUM_JAIL -s 2001:db8::1/128 -j DROP"),
            Some("2001:db8::1".parse().unwrap())
        );
        assert_eq!(Jail::extract_ip("-A ATRIUM_JAIL -j DROP"), None);
        assert_eq!(
            Jail::extract_ip("-A ATRIUM_JAIL -s malformed -j DROP"),
            None
        );
    }

    #[test]
    fn test_extract_timestamp() {
        assert_eq!(
            Jail::extract_timestamp(
                "-A ATRIUM_JAIL -s 1.2.3.4/32 -j DROP -m comment --comment 1731424000"
            ),
            Some(1731424000)
        );
        assert_eq!(
            Jail::extract_timestamp(
                "-A ATRIUM_JAIL -s 1.2.3.4/32 -j DROP -m comment --comment \"1731424000\""
            ),
            Some(1731424000)
        );
        assert_eq!(
            Jail::extract_timestamp("-A ATRIUM_JAIL -s 1.2.3.4/32 -j DROP"),
            None
        );
        assert_eq!(Jail::extract_timestamp("-m comment --comment abc"), None);
    }

    #[test]
    fn test_update_failures() {
        let mut failures = Vec::new();
        let now = Instant::now();
        let find_time = Duration::from_secs(60);
        let max_retry = 3;

        // First failure
        assert!(!Jail::update_failures(
            &mut failures,
            now,
            find_time,
            max_retry
        ));
        assert_eq!(failures.len(), 1);

        // Second failure
        assert!(!Jail::update_failures(
            &mut failures,
            now + Duration::from_secs(10),
            find_time,
            max_retry
        ));
        assert_eq!(failures.len(), 2);

        // Third failure -> triggers ban
        assert!(Jail::update_failures(
            &mut failures,
            now + Duration::from_secs(20),
            find_time,
            max_retry
        ));
        assert_eq!(failures.len(), 3);

        // Reset and test expiration
        failures.clear();
        assert!(!Jail::update_failures(
            &mut failures,
            now,
            find_time,
            max_retry
        ));
        assert!(!Jail::update_failures(
            &mut failures,
            now + Duration::from_secs(70),
            find_time,
            max_retry
        ));
        // The first one should be purged
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn test_generate_ban_rule() {
        let ip_v4: IpAddr = "1.2.3.4".parse().unwrap();
        let (rule, is_ipv6) = Jail::generate_ban_rule(ip_v4, 12345);
        assert_eq!(rule, "-s 1.2.3.4 -j DROP -m comment --comment 12345");
        assert!(!is_ipv6);

        let ip_v6: IpAddr = "2001:db8::1".parse().unwrap();
        let (rule, is_ipv6) = Jail::generate_ban_rule(ip_v6, 12345);
        assert_eq!(rule, "-s 2001:db8::1 -j DROP -m comment --comment 12345");
        assert!(is_ipv6);

        // Test IPv4-mapped IPv6
        let ip_v4_mapped: IpAddr =
            Jail::normalize_ip(IpAddr::V6(Ipv4Addr::new(1, 2, 3, 4).to_ipv6_mapped()));
        let (rule, is_ipv6) = Jail::generate_ban_rule(ip_v4_mapped, 12345);
        assert_eq!(rule, "-s 1.2.3.4 -j DROP -m comment --comment 12345");
        assert!(!is_ipv6);
    }

    #[test]
    fn test_jail_config_defaults() {
        let config = JailConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.max_retry, 3);
        assert_eq!(config.find_time, 60);
        assert_eq!(config.ban_time, 30);
        assert!(config.whitelist.is_empty());
    }

    #[test]
    fn test_jail_config_whitelist() {
        let ip_v4: IpAddr = "1.2.3.4".parse().unwrap();
        let ip_v6: IpAddr = "2001:db8::1".parse().unwrap();
        let config = JailConfig {
            enabled: true,
            whitelist: vec![ip_v4, ip_v6],
            ..Default::default()
        };

        assert!(config.whitelist.contains(&ip_v4));
        assert!(config.whitelist.contains(&ip_v6));
        assert!(!config.whitelist.contains(&"8.8.8.8".parse().unwrap()));
    }

    #[tokio::test]
    async fn test_jail_disabled() {
        let config = JailConfig {
            enabled: false,
            ..Default::default()
        };
        let jail = Jail::new_from_config(&config).await;
        assert!(jail.is_none());
    }
}
