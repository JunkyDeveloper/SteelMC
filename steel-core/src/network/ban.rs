//! IP ban / whitelist / blacklist management.
//!
//! Persisted in `config/banned.json`, `config/whitelist.json`, and `config/blacklist.json`.
//! Access the global manager via [`IP_ACCESS_POLICY`].

use chrono::{DateTime, Utc};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::IpAddr;
use std::path::Path;
use std::sync::LazyLock;
use std::{fs, io};
use steel_utils::locks::SyncRwLock;

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S %z";
const FOREVER: &str = "forever";

const BANNED_PATH: &str = "config/banned.json";
const WHITELIST_PATH: &str = "config/whitelist.json";
const BLACKLIST_PATH: &str = "config/blacklist.json";

/// Serde adapter for a required timestamp, formatted as `"YYYY-MM-DD HH:MM:SS +0000"`.
#[allow(dead_code)]
mod local_datetime_format {
    use super::DATETIME_FORMAT;
    use chrono::{DateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", dt.format(DATETIME_FORMAT));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DateTime::parse_from_str(&s, DATETIME_FORMAT)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }
}

/// Serde adapter for an optional expiry timestamp. `None` is encoded as the
/// literal string `"forever"`, matching vanilla's permanent-ban representation.
#[allow(dead_code)]
mod local_datetime_or_forever_format {
    use super::{DATETIME_FORMAT, FOREVER};
    use chrono::{DateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(dt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match dt {
            Some(dt) => serializer.serialize_str(&format!("{}", dt.format(DATETIME_FORMAT))),
            None => serializer.serialize_str(FOREVER),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s == FOREVER {
            return Ok(None);
        }
        DateTime::parse_from_str(&s, DATETIME_FORMAT)
            .map(|dt| Some(dt.with_timezone(&Utc)))
            .map_err(serde::de::Error::custom)
    }
}

/// A single entry in `banned.json`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BannedIP {
    /// The banned address.
    pub ip: IpAddr,
    /// When the ban was issued.
    #[serde(with = "local_datetime_format")]
    pub created: DateTime<Utc>,
    /// Who or what issued the ban (e.g. operator name, `"start"` for the
    /// startup demo ban).
    pub source: String,
    /// When the ban expires. `None` means the ban never expires.
    #[serde(with = "local_datetime_or_forever_format")]
    pub expires: Option<DateTime<Utc>>,
    /// Operator-provided reason shown to the banned client.
    pub reason: String,
}

// TODO: DELETE ME
/// Extends vanilla's `commands.banlist.entry` format (`<ip> was banned by
/// <source>: <reason>`) with the expiry, since this rendering is used for
/// operator-facing logs where the expiry is the relevant info.
impl fmt::Display for BannedIP {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} was banned by {} (expires ", self.ip, self.source)?;
        match self.expires {
            Some(expires) => write!(f, "{}", expires.format(DATETIME_FORMAT))?,
            None => f.write_str(FOREVER)?,
        }
        write!(f, "): {}", self.reason)
    }
}

/// Internal state bundle held behind [`IpAccessPolicy`]'s `SyncRwLock`.
struct IpAccessPolicyState {
    banned_ips_config_all: Vec<BannedIP>,
    banned_ips: FxHashSet<IpAddr>,
    blacklisted_ips: FxHashSet<IpAddr>,
    white_list_ips: FxHashSet<IpAddr>,
    // less cpu cycles then calling vec.len() every time in loop
    has_whitelist: bool,
}

/// Thread-safe holder for ban / whitelist / blacklist state.
///
/// All public methods take `&self`; mutation goes through an internal
/// `SyncRwLock`. Use the global [`IP_ACCESS_POLICY`] static — there is no reason
/// to construct a second instance outside of tests.
pub struct IpAccessPolicy {
    state: SyncRwLock<IpAccessPolicyState>,
}

/// Global IP ban manager.
///
/// Auto-initializes on first access by loading `config/whitelist.json`,
/// `config/banned.json`, and `config/blacklist.json`.
pub static IP_ACCESS_POLICY: LazyLock<IpAccessPolicy> = LazyLock::new(|| {
    let manager = IpAccessPolicy::empty();
    manager.load_whitelisted_ips();
    manager.load_banned_ips();
    manager.load_blacklisted_ips();
    manager
});

impl IpAccessPolicy {
    /// Builds a manager with no entries loaded. Used as the starting point
    /// for [`IP_ACCESS_POLICY`] before the JSON files are read.
    pub fn empty() -> Self {
        Self {
            state: SyncRwLock::new(IpAccessPolicyState {
                banned_ips: FxHashSet::default(),
                banned_ips_config_all: Vec::default(),
                white_list_ips: FxHashSet::default(),
                blacklisted_ips: FxHashSet::default(),
                has_whitelist: false,
            }),
        }
    }

    /// Removes any bans whose `expires` timestamp has passed.
    ///
    /// Deviates from vanilla: `StoredUserList.get` prunes on every lookup,
    /// paying the cost on the hot path. Steel keeps a separate `banned_ips`
    /// HashSet so [`is_banned`](Self::is_banned) stays an O(1) check, and
    /// shifts the pruning work onto a periodic call to this method instead.
    pub fn expire_bans(&self) {
        let now = Utc::now();
        let mut state = self.state.write();
        state
            .banned_ips_config_all
            .retain(|b| b.expires.is_none_or(|t| t > now));
        state.banned_ips = state.banned_ips_config_all.iter().map(|b| b.ip).collect();
    }

    /// Adds a ban for `ip`. Persisted only when [`save_config`](Self::save_config) runs.
    ///
    /// # Panics
    /// Panics if `ip` is not a valid IP address literal.
    pub fn ban_ip(
        &self,
        ip: String,
        source: String,
        reason: String,
        expires: Option<DateTime<Utc>>,
    ) {
        let parsed: IpAddr = ip.parse().unwrap();
        let mut state = self.state.write();
        state.banned_ips_config_all.push(BannedIP {
            ip: parsed,
            created: Utc::now(),
            source,
            expires,
            reason,
        });
        state.banned_ips.insert(parsed);
    }

    /// Removes `ip` from the ban list. Persisted only when [`save_config`](Self::save_config) runs.
    pub fn un_ban_ip(&self, ip: &IpAddr) {
        let mut state = self.state.write();
        state.banned_ips.remove(ip);
        state.banned_ips_config_all.retain(|b| b.ip != *ip);
    }
    /// Removes `ip` from the blacklist. Persisted only when [`save_config`](Self::save_config) runs.
    pub fn un_blacklist_ip(&self, ip: &IpAddr) {
        let mut state = self.state.write();
        state.blacklisted_ips.remove(ip);
    }

    /// Returns a snapshot of all currently whitelisted IPs.
    pub fn get_whitelist_ips(&self) -> Vec<IpAddr> {
        self.state
            .read()
            .white_list_ips
            .iter()
            .map(|ip| ip.clone())
            .collect()
    }

    /// Returns a snapshot of all current ban entries, including metadata.
    pub fn get_banned_ips(&self) -> Vec<BannedIP> {
        self.state.read().banned_ips_config_all.clone()
    }
    /// Returns a snapshot of all currently blacklisted IPs.
    pub fn get_blacklisted_ips(&self) -> Vec<IpAddr> {
        self.state
            .read()
            .blacklisted_ips
            .iter()
            .map(|ip| ip.clone())
            .collect()
    }

    /// Adds `ip` to the whitelist. Persisted only when
    /// [`save_config`](Self::save_config) runs.
    ///
    /// # Panics
    /// Panics if `ip` is not a valid IP address literal.
    pub fn white_list_ip(&self, ip: String) {
        self.state
            .write()
            .white_list_ips
            .insert(ip.parse().unwrap());
    }

    /// Reloads the ban list from `config/banned.json`, replacing the current set.
    ///
    /// Creates the file with `[]` if it does not exist. On parse error,
    /// logs and keeps the existing in-memory state.
    pub fn load_banned_ips(&self) {
        let path = Path::new(BANNED_PATH);

        let raw = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                tracing::warn!("{BANNED_PATH} not found — creating empty file");
                fs::create_dir_all(path.parent().unwrap_or(Path::new("config")))
                    .unwrap_or_else(|e| tracing::error!("Failed to create config dir: {}", e));
                fs::write(path, "[]")
                    .unwrap_or_else(|e| tracing::error!("Failed to create {BANNED_PATH}: {}", e));
                "[]".to_string()
            }
            Err(e) => {
                tracing::error!("Failed to read {BANNED_PATH}: {}", e);
                return;
            }
        };

        match serde_json::from_str::<Vec<BannedIP>>(&raw) {
            Ok(banned_ips) => {
                let count = banned_ips.len();
                let mut state = self.state.write();
                state.banned_ips.clear();
                for ip in &banned_ips {
                    state.banned_ips.insert(ip.ip);
                }
                state.banned_ips_config_all = banned_ips;
                drop(state);
                tracing::info!("Loaded {} banned IPs", count);
            }
            Err(e) => tracing::error!("{BANNED_PATH} invalid JSON, keeping previous state: {}", e),
        }
    }

    /// Reloads the whitelist from `config/whitelist.json`, replacing the current set.
    ///
    /// Creates the file with `[]` if it does not exist. On parse error,
    /// logs and keeps the existing in-memory state.
    pub fn load_whitelisted_ips(&self) {
        let path = Path::new(WHITELIST_PATH);

        let raw = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                tracing::warn!("{WHITELIST_PATH} not found — creating empty file");
                fs::create_dir_all(path.parent().unwrap_or(Path::new("config")))
                    .unwrap_or_else(|e| tracing::error!("Failed to create config dir: {}", e));
                fs::write(path, "[]").unwrap_or_else(|e| {
                    tracing::error!("Failed to create {WHITELIST_PATH}: {}", e)
                });
                "[]".to_string()
            }
            Err(e) => {
                tracing::error!("Failed to read {WHITELIST_PATH}: {}", e);
                return;
            }
        };

        match serde_json::from_str::<FxHashSet<IpAddr>>(&raw) {
            Ok(white_list) => {
                let count = white_list.len();
                self.state.write().white_list_ips = white_list;
                tracing::info!("Loaded {} white listed IPs", count);
            }
            Err(e) => tracing::error!(
                "{WHITELIST_PATH} invalid JSON, keeping previous state: {}",
                e
            ),
        }
    }

    /// Reloads the blacklist from `config/blacklist.json`, replacing the current set.
    ///
    /// Creates the file with `[]` if it does not exist. On parse error,
    /// logs and keeps the existing in-memory state.
    ///
    /// The blacklist gates connections at accept time
    /// ([`can_join_preconnecting`](Self::can_join_preconnecting)); the ban
    /// list gates them later ([`can_join`](Self::is_banned)).
    pub fn load_blacklisted_ips(&self) {
        let path = Path::new(BLACKLIST_PATH);

        let raw = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                tracing::warn!("{BLACKLIST_PATH} not found — creating empty file");
                fs::create_dir_all(path.parent().unwrap_or(Path::new("config")))
                    .unwrap_or_else(|e| tracing::error!("Failed to create config dir: {}", e));
                fs::write(path, "[]").unwrap_or_else(|e| {
                    tracing::error!("Failed to create {BLACKLIST_PATH}: {}", e)
                });
                "[]".to_string()
            }
            Err(e) => {
                tracing::error!("Failed to read {BLACKLIST_PATH}: {}", e);
                return;
            }
        };

        match serde_json::from_str::<FxHashSet<IpAddr>>(&raw) {
            Ok(black_list) => {
                let count = black_list.len();
                self.state.write().blacklisted_ips = black_list;
                tracing::info!("Loaded {} blacklisted IPs", count);
            }
            Err(e) => tracing::error!(
                "{BLACKLIST_PATH} invalid JSON, keeping previous state: {}",
                e
            ),
        }
    }

    /// Whether `address` may complete the TCP accept stage.
    ///
    /// If a whitelist is active, only whitelisted IPs pass. Otherwise, only
    /// non-blacklisted IPs pass. Used by the accept loop to drop banned
    /// connections without going through the login handshake.
    pub fn can_join_preconnecting(&self, ip: &IpAddr) -> bool {
        let state = self.state.read();
        (state.has_whitelist && state.white_list_ips.contains(ip))
            || (!state.has_whitelist && !state.blacklisted_ips.contains(ip))
    }

    /// Whether `address` may join the game.
    ///
    /// If a whitelist is active, only whitelisted IPs pass. Otherwise, only
    /// non-banned IPs pass. Checked after the connection has progressed past
    /// the accept stage.
    pub fn is_banned(&self, ip: &IpAddr) -> bool {
        let state = self.state.read();
        (state.has_whitelist && !state.white_list_ips.contains(ip))
            || (!state.has_whitelist && state.banned_ips.contains(ip))
    }

    /// Returns the ban reason for `ip`, or `None` if the IP is not in the ban list.
    pub fn get_banned_reason(&self, ip: &IpAddr) -> Option<String> {
        let state = self.state.read();
        state
            .banned_ips_config_all
            .iter()
            .find(|b| b.ip == *ip)
            .map(|b| b.reason.clone())
    }

    /// Persists ban list, whitelist, and blacklist to their JSON files.
    ///
    /// Called automatically from `SteelServer`'s `Drop` impl on clean shutdown.
    ///
    /// # Panics
    /// Panics if serialization or file write fails.
    pub fn save_config(&self) {
        fs::create_dir_all("config").expect("Failed to create config dir");
        let state = self.state.read();
        let json_out = serde_json::to_string_pretty(&state.banned_ips_config_all)
            .expect("Failed to serialize banned_ips_config_all");
        fs::write(BANNED_PATH, &json_out).expect("Failed to write config/banned.json");
        let json_out = serde_json::to_string_pretty(&state.white_list_ips)
            .expect("Failed to serialize white_list_ips");
        fs::write(WHITELIST_PATH, &json_out).expect("Failed to write config/whitelist.json");
        let json_out = serde_json::to_string_pretty(&state.blacklisted_ips)
            .expect("Failed to serialize blacklisted_ips");
        fs::write(BLACKLIST_PATH, &json_out).expect("Failed to write config/blacklist.json");
    }
}
