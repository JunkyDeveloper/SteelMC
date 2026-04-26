//! Level data persistence module.
//!
//! This module handles saving and loading world-level data like game rules,
//! time, weather, spawn point, and seed. This data is stored in `level.json`
//! in each world's directory.

use chrono::{DateTime, Local, Utc};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::LazyLock;
use std::{fs, io};

#[allow(dead_code)]
mod local_datetime_format {
    use chrono::{DateTime, Local, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &str = "%Y-%m-%d %H:%M:%S %z";

    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", dt.format(FORMAT));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // Parse as a DateTime with a fixed offset, then convert to Local.
        DateTime::parse_from_str(&s, FORMAT)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BannedIP {
    pub ip: IpAddr,
    #[serde(with = "local_datetime_format")]
    pub created: DateTime<Utc>,
    pub source: String,
    #[serde(with = "local_datetime_format")]
    pub expires: DateTime<Utc>,
    pub reason: String,
}

// pub static IP_MANAGER: LazyLock<IPBanManager> = LazyLock::new(|| init());

pub struct IPBanManager {
    pub banned_ips_config_all: Vec<BannedIP>,
    banned_ips: FxHashSet<IpAddr>,
    white_list_ips: FxHashSet<IpAddr>,
    // less cpu cycles then calling vec.len() every time in loop
    has_whitelist: bool,
}
pub fn init() -> IPBanManager {
    let mut manager = IPBanManager::empty();
    manager.load_whitelisted_ips();
    manager.load_banned_ips();
    manager
}

impl IPBanManager {
    pub fn empty() -> Self {
        Self {
            banned_ips: FxHashSet::default(),
            banned_ips_config_all: Vec::default(),
            white_list_ips: FxHashSet::default(),
            has_whitelist: false,
        }
    }

    pub fn expire_bans(&mut self) {}

    pub fn ban_ip(&mut self, ip: String, source: String, reason: String, expires: DateTime<Utc>) {
        self.banned_ips_config_all.push(BannedIP {
            ip: ip.parse().unwrap(),
            created: Utc::now(),
            source,
            expires,
            reason,
        });
        self.banned_ips.insert(ip.parse().unwrap());
    }

    pub fn white_list_ip(&mut self, ip: String) {
        self.white_list_ips.insert(ip.parse().unwrap());
    }

    pub fn load_banned_ips(&mut self) {
        let path = Path::new("banned.json");

        let raw = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                tracing::warn!("banned.json not found — creating empty file");
                fs::write(path, "[]")
                    .unwrap_or_else(|e| tracing::error!("Failed to create banned.json: {}", e));
                "[]".to_string()
            }
            Err(e) => {
                tracing::error!("Failed to read banned.json: {}", e);
                return;
            }
        };

        match serde_json::from_str(&raw) {
            Ok(banned_ips) => {
                self.banned_ips_config_all = banned_ips;
                for ip in &self.banned_ips_config_all {
                    self.banned_ips.insert(ip.ip);
                }
            }
            Err(e) => tracing::error!("banned.json invalid JSON, keeping previous state: {}", e),
        }
        tracing::info!(
            "Loaded {} banned IPs",
            self.banned_ips_config_all.len()
        );
    }

    pub fn load_whitelisted_ips(&mut self) {
        let path = Path::new("whitelist.json");

        let raw = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                tracing::warn!("whitelist.json not found — creating empty file");
                fs::write(path, "[]")
                    .unwrap_or_else(|e| tracing::error!("Failed to create whitelist.json: {}", e));
                "[]".to_string()
            }
            Err(e) => {
                tracing::error!("Failed to read whitelist.json: {}", e);
                return;
            }
        };

        match serde_json::from_str(&raw) {
            Ok(white_list) => self.white_list_ips = white_list,
            Err(e) => tracing::error!("whitelist.json invalid JSON, keeping previous state: {}", e),
        }
        tracing::info!("Loaded {} white listed IPs", self.white_list_ips.len());
    }

    pub fn can_join(&self, address: SocketAddr) -> bool {
        (self.has_whitelist && self.white_list_ips.contains(&address.ip()))
            || self.banned_ips.contains(&address.ip())
    }

    pub fn save_config(&self) {
        let json_out = serde_json::to_string_pretty(&self.banned_ips_config_all)
            .expect("Failed to serialize banned_ips_config_all");
        fs::write("banned.json", &json_out).expect("Failed to write user_updated.json");
        let json_out = serde_json::to_string_pretty(&self.white_list_ips)
            .expect("Failed to serialize white_list_ips");
        fs::write("whitelist.json", &json_out).expect("Failed to write whitelist.json");
    }
}
