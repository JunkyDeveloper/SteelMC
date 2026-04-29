//! IP ban / whitelist / blacklist management.
//!
//! Persisted in `config/ip-bans.toml`. The whitelist is sourced from the
//! server config (see [`init_ip_access_policy`]). Vanilla's
//! `config/banned-ips.json` is read once on first startup if no Steel ban
//! file exists yet.
//!
//! Access the global manager via [`IP_ACCESS_POLICY`].

use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDate, TimeZone, Timelike, Utc};
use rustc_hash::FxHashSet;
use serde::de::Error as DeError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::IpAddr;
use std::ops::Deref;
use std::path::Path;
use std::sync::OnceLock;
use std::{fs, io};
use steel_utils::locks::SyncRwLock;
use steel_utils::translations::{
    MULTIPLAYER_DISCONNECT_BANNED_IP_EXPIRATION, MULTIPLAYER_DISCONNECT_BANNED_IP_REASON,
};
use text_components::{Modifier, TextComponent};
use toml::value;

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S %z";
const FOREVER: &str = "forever";
/// Substituted as the ban reason when a vanilla `banned-ips.json` entry has
/// no reason or a `null` / empty one. Steel always stores a reason; this
/// string is the user-visible message a vanilla-imported ban falls back to.
const DEFAULT_BAN_REASON: &str = "Your IP was banned";

const BANS_PATH: &str = "config/ip-bans.toml";
const MINECRAFT_BANNED_IP_PATH: &str = "config/banned-ips.json";

/// Empty `ip-bans.toml` written when no ban file exists and no vanilla
/// import was available. Kept in sync with [`IpBansFile`]'s field names.
const EMPTY_BANS_TOML: &str = "ip_banned = []\nblacklisted = []\n";

fn parse_datetime<E: DeError>(s: &str) -> Result<DateTime<Utc>, E> {
    DateTime::parse_from_str(s, DATETIME_FORMAT)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(E::custom)
}

/// Builds a UTC [`value::Datetime`] from a chrono UTC timestamp so the
/// `toml` crate emits a real RFC 3339 datetime literal (e.g.
/// `2026-04-29T22:05:51Z`) instead of a quoted string.
fn chrono_to_toml_datetime(dt: &DateTime<Utc>) -> value::Datetime {
    value::Datetime {
        date: Some(value::Date {
            year: dt.year() as u16,
            month: dt.month() as u8,
            day: dt.day() as u8,
        }),
        time: Some(value::Time {
            hour: dt.hour() as u8,
            minute: dt.minute() as u8,
            second: dt.second() as u8,
            nanosecond: dt.nanosecond(),
        }),
        offset: Some(value::Offset::Z),
    }
}

/// Converts a parsed [`value::Datetime`] back into a chrono UTC instant.
/// Local (offset-less) datetimes are interpreted as UTC, matching what we
/// emit. Returns `None` if the value is missing date/time or carries an
/// out-of-range field.
fn toml_datetime_to_chrono(dt: &value::Datetime) -> Option<DateTime<Utc>> {
    let date = dt.date?;
    let time = dt.time?;
    let naive = NaiveDate::from_ymd_opt(date.year.into(), date.month.into(), date.day.into())?
        .and_hms_nano_opt(
            time.hour.into(),
            time.minute.into(),
            time.second.into(),
            time.nanosecond,
        )?;
    let offset_secs = match dt.offset {
        Some(value::Offset::Z) | None => 0,
        Some(value::Offset::Custom { minutes }) => i32::from(minutes) * 60,
    };
    let fixed = FixedOffset::east_opt(offset_secs)?;
    let with_offset = fixed.from_local_datetime(&naive).single()?;
    Some(with_offset.with_timezone(&Utc))
}

/// Serde adapter for a required UTC timestamp encoded as a TOML-native
/// datetime (unquoted RFC 3339). Used by Steel's own `ip-bans.toml`; the
/// vanilla import path keeps the legacy string-based [`vanilla_datetime_format`].
mod toml_datetime_format {
    use super::{chrono_to_toml_datetime, toml_datetime_to_chrono};
    use chrono::{DateTime, Utc};
    use serde::de::Error as DeError;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use toml::value;

    pub fn serialize<S: Serializer>(dt: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        chrono_to_toml_datetime(dt).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        let toml_dt = value::Datetime::deserialize(d)?;
        toml_datetime_to_chrono(&toml_dt).ok_or_else(|| D::Error::custom("invalid datetime"))
    }
}

/// Serde adapter for an optional expiry encoded as either a TOML-native
/// datetime or the literal string `"forever"` for permanent bans. Steel-only;
/// vanilla imports use [`vanilla_datetime_or_forever_format`].
mod toml_datetime_or_forever_format {
    use super::{FOREVER, chrono_to_toml_datetime, toml_datetime_to_chrono};
    use chrono::{DateTime, Utc};
    use serde::de::Error as DeError;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use toml::value;

    #[expect(
        clippy::ref_option,
        reason = "serde's #[serde(with = ...)] machinery calls serialize(&field, ...), so the parameter must be &Option<T>"
    )]
    pub fn serialize<S: Serializer>(dt: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error> {
        match dt {
            Some(dt) => chrono_to_toml_datetime(dt).serialize(s),
            None => s.serialize_str(FOREVER),
        }
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DatetimeOrForever {
        Datetime(value::Datetime),
        Forever(String),
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
        match DatetimeOrForever::deserialize(d)? {
            DatetimeOrForever::Datetime(toml_dt) => toml_datetime_to_chrono(&toml_dt)
                .map(Some)
                .ok_or_else(|| D::Error::custom("invalid datetime")),
            DatetimeOrForever::Forever(s) if s == FOREVER => Ok(None),
            DatetimeOrForever::Forever(s) => Err(D::Error::custom(format!(
                "expected a datetime or {FOREVER:?}, got {s:?}"
            ))),
        }
    }
}

/// Serde deserializer for vanilla's `"YYYY-MM-DD HH:MM:SS +0000"` timestamp
/// strings. Read-only — Steel never writes this format.
mod vanilla_datetime_format {
    use super::parse_datetime;
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        parse_datetime(&String::deserialize(d)?)
    }
}

/// Serde adapter that round-trips a [`TextComponent`] through its JSON
/// representation as a single string. Without this, `toml::to_string_pretty`
/// renders `reason` as a `[ip_banned.reason]` sub-table, which (with multiple
/// ban entries) emits the same header twice and fails to reparse.
mod reason_json_string_format {
    use serde::de::Error as DeError;
    use serde::{Deserialize, Deserializer, Serializer};
    use text_components::TextComponent;

    pub fn serialize<S: Serializer>(reason: &TextComponent, s: S) -> Result<S::Ok, S::Error> {
        let json = serde_json::to_string(reason).map_err(serde::ser::Error::custom)?;
        s.serialize_str(&json)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<TextComponent, D::Error> {
        let s = String::deserialize(d)?;
        serde_json::from_str(&s).map_err(D::Error::custom)
    }
}

/// Serde deserializer for vanilla's optional expiry — either a
/// `"YYYY-MM-DD HH:MM:SS +0000"` string or the literal `"forever"`.
/// Read-only — Steel never writes this format.
mod vanilla_datetime_or_forever_format {
    use super::{FOREVER, parse_datetime};
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
        let s = String::deserialize(d)?;
        if s == FOREVER {
            return Ok(None);
        }
        parse_datetime(&s).map(Some)
    }
}

/// A single entry in `banned.json`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BannedIP {
    /// The banned address.
    pub ip: IpAddr,
    /// When the ban was issued.
    #[serde(with = "toml_datetime_format")]
    pub created: DateTime<Utc>,
    /// Who or what issued the ban (e.g. operator name, `"start"` for the
    /// startup demo ban).
    pub source: String,
    /// When the ban expires. `None` means the ban never expires.
    #[serde(with = "toml_datetime_or_forever_format")]
    pub expires: Option<DateTime<Utc>>,
    /// Operator-provided reason shown to the banned client.
    #[serde(with = "reason_json_string_format")]
    pub reason: TextComponent,
}

/// On-disk shape of `config/ip-bans.toml`. Holds both the metadata-rich
/// ban list and the bare blacklist. In-memory the blacklist is a
/// `FxHashSet`; TOML has no native set type so it round-trips through
/// `Vec<IpAddr>`.
#[derive(Serialize, Deserialize, Default)]
struct IpBansFile {
    ip_banned: Vec<BannedIP>,
    blacklisted: Vec<IpAddr>,
}

/// Read-only mirror of [`BannedIP`] used when importing vanilla's
/// `banned-ips.json`. Vanilla writes a `reason` that may be missing or `null`,
/// which is foreign to Steel's canonical schema; converting through this
/// helper substitutes [`DEFAULT_BAN_REASON`] without polluting `BannedIP`.
#[derive(Deserialize)]
struct VanillaBannedIp {
    ip: IpAddr,
    #[serde(deserialize_with = "vanilla_datetime_format::deserialize")]
    created: DateTime<Utc>,
    source: String,
    #[serde(deserialize_with = "vanilla_datetime_or_forever_format::deserialize")]
    expires: Option<DateTime<Utc>>,
    #[serde(default)]
    reason: Option<TextComponent>,
}

impl From<VanillaBannedIp> for BannedIP {
    fn from(v: VanillaBannedIp) -> Self {
        Self {
            ip: v.ip,
            created: v.created,
            source: v.source,
            expires: v.expires,
            reason: v
                .reason
                .filter(|s| !s.to_string().is_empty())
                .unwrap_or_else(|| TextComponent::plain(DEFAULT_BAN_REASON)),
        }
    }
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

/// Wrapper for the global IP access policy that implements `Deref`.
pub struct IpAccessPolicyLock(OnceLock<IpAccessPolicy>);

impl Deref for IpAccessPolicyLock {
    type Target = IpAccessPolicy;

    fn deref(&self) -> &Self::Target {
        self.0.get().expect("IP access policy not initialized")
    }
}

/// Global IP access policy.
///
/// Populated by [`init_ip_access_policy`] at startup with the whitelist taken
/// from the server config; bans are loaded from `config/ip-bans.json`.
pub static IP_ACCESS_POLICY: IpAccessPolicyLock = IpAccessPolicyLock(OnceLock::new());

/// Initializes the global [`IP_ACCESS_POLICY`].
///
/// Builds the policy from the provided whitelist and the on-disk ban list.
/// Should be called once during server startup, before any code path that
/// accesses [`IP_ACCESS_POLICY`] (e.g. the TCP accept loop).
///
/// # Panics
///
/// Panics if called more than once.
pub fn init_ip_access_policy(white_listed_ips: &[IpAddr]) {
    let policy = IpAccessPolicy::empty();
    policy.set_whitelist(white_listed_ips);
    policy.load_bans();
    assert!(
        IP_ACCESS_POLICY.0.set(policy).is_ok(),
        "init_ip_access_policy called more than once"
    );
}

impl IpAccessPolicy {
    /// Builds a manager with no entries loaded. Used as the starting point
    /// for [`IP_ACCESS_POLICY`] before the JSON files are read.
    #[must_use]
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
    /// `HashSet` so [`is_banned`](Self::is_banned) stays an O(1) check, and
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
    /// Return false is the ip is already banned, else true
    pub fn ban_ip(
        &self,
        ip: IpAddr,
        source: String,
        reason: TextComponent,
        expires: Option<DateTime<Utc>>,
    ) -> bool {
        if self.is_banned(&ip) {
            return false;
        }
        let mut state = self.state.write();
        state.banned_ips_config_all.push(BannedIP {
            ip,
            created: Utc::now(),
            source,
            expires,
            reason,
        });
        state.banned_ips.insert(ip);

        true
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

    /// Adds `ip` from the blacklist. Persisted only when [`save_config`](Self::save_config) runs.
    /// Return false is the ip is already blacklisted, else true
    pub fn blacklist_ip(&self, ip: &IpAddr) -> bool {
        if self.is_blacklisted(ip) {
            return false;
        }
        let mut state = self.state.write();
        state.blacklisted_ips.insert(*ip);
        true
    }

    /// Returns a snapshot of all currently whitelisted IPs.
    pub fn get_whitelist_ips(&self) -> Vec<IpAddr> {
        self.state.read().white_list_ips.iter().copied().collect()
    }

    /// Returns a snapshot of all current ban entries, including metadata.
    pub fn get_banned_ips(&self) -> Vec<BannedIP> {
        self.state.read().banned_ips_config_all.clone()
    }
    /// Returns a snapshot of all currently blacklisted IPs.
    pub fn get_blacklisted_ips(&self) -> Vec<IpAddr> {
        self.state.read().blacklisted_ips.iter().copied().collect()
    }

    /// Reloads the ban list and blacklist from `config/ip-bans.toml`.
    ///
    /// If `ip-bans.toml` is missing, attempts a one-time import from
    /// vanilla's `config/banned-ips.json` (which only populates the ban
    /// list — vanilla has no separate blacklist concept). If neither
    /// exists, creates an empty `ip-bans.toml`. On parse error, logs and
    /// keeps the in-memory state.
    pub fn load_bans(&self) {
        let path = Path::new(BANS_PATH);

        match fs::read_to_string(path) {
            Ok(raw) => match toml::from_str::<IpBansFile>(&raw) {
                Ok(file) => self.apply_loaded_bans_file(file, BANS_PATH),
                Err(e) => {
                    tracing::error!("{BANS_PATH} invalid TOML, keeping previous state: {}", e);
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if self.try_import_vanilla_bans() {
                    return;
                }
                tracing::warn!("{BANS_PATH} not found — creating empty file");
                fs::create_dir_all(path.parent().unwrap_or(Path::new("config")))
                    .unwrap_or_else(|e| tracing::error!("Failed to create config dir: {}", e));
                fs::write(path, EMPTY_BANS_TOML)
                    .unwrap_or_else(|e| tracing::error!("Failed to create {BANS_PATH}: {}", e));
            }
            Err(e) => tracing::error!("Failed to read {BANS_PATH}: {}", e),
        }
    }

    /// Tries to populate the ban list from vanilla's `banned-ips.json`.
    /// Returns `true` if entries were imported (so the caller skips the
    /// empty-file fallback). Any failure mode logs and returns `false`.
    pub fn try_import_vanilla_bans(&self) -> bool {
        let raw = match fs::read_to_string(MINECRAFT_BANNED_IP_PATH) {
            Ok(raw) => raw,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return false,
            Err(e) => {
                tracing::error!("Failed to read {MINECRAFT_BANNED_IP_PATH}: {}", e);
                return false;
            }
        };
        match serde_json::from_str::<Vec<VanillaBannedIp>>(&raw) {
            Ok(vanilla_entries) => {
                let banned: Vec<BannedIP> =
                    vanilla_entries.into_iter().map(BannedIP::from).collect();
                tracing::info!(
                    "{BANS_PATH} not found — importing {} entries from vanilla {MINECRAFT_BANNED_IP_PATH}; will be saved to {BANS_PATH} on next save",
                    banned.len()
                );
                self.apply_loaded_bans_file(
                    IpBansFile {
                        ip_banned: banned,
                        blacklisted: Vec::new(),
                    },
                    MINECRAFT_BANNED_IP_PATH,
                );
                true
            }
            Err(e) => {
                tracing::error!(
                    "{MINECRAFT_BANNED_IP_PATH} invalid JSON, skipping vanilla import: {}",
                    e
                );
                false
            }
        }
    }

    /// Replaces in-memory ban + blacklist state with `file`, rebuilding the
    /// fast lookup sets. Logs the counts under `source` for operator visibility.
    fn apply_loaded_bans_file(&self, file: IpBansFile, source: &str) {
        let banned_count = file.ip_banned.len();
        let blacklisted_count = file.blacklisted.len();
        let mut state = self.state.write();
        state.banned_ips = file.ip_banned.iter().map(|b| b.ip).collect();
        state.banned_ips_config_all = file.ip_banned;
        state.blacklisted_ips = file.blacklisted.into_iter().collect();
        drop(state);
        tracing::info!(
            "Loaded {banned_count} banned IPs and {blacklisted_count} blacklisted IPs from {source}"
        );
    }

    /// Replaces the in-memory whitelist with `ips`.
    fn set_whitelist(&self, ips: &[IpAddr]) {
        let mut state = self.state.write();
        state.white_list_ips = ips.iter().copied().collect();
        state.has_whitelist = !ips.is_empty();
    }

    /// Whether `ip` may complete the TCP accept stage.
    ///
    /// If a whitelist is active, only whitelisted IPs pass. Otherwise, only
    /// non-blacklisted IPs pass. Used by the accept loop to drop banned
    /// connections without going through the login handshake.
    pub fn can_connect(&self, ip: &IpAddr) -> bool {
        let state = self.state.read();
        (state.has_whitelist && state.white_list_ips.contains(ip))
            || (!state.has_whitelist && !state.blacklisted_ips.contains(ip))
    }

    /// Whether `ip` may join the game.
    ///
    /// If a whitelist is active, only whitelisted IPs pass. Otherwise, only
    /// non-banned IPs pass. Checked after the connection has progressed past
    /// the accept stage.
    pub fn is_banned(&self, ip: &IpAddr) -> bool {
        let state = self.state.read();
        state.banned_ips.contains(ip)
    }

    /// Whether `ip` can see the server is online in the server list.
    ///
    /// If an ip is blacklisted, they cannot see the server online in the server list,
    /// they also can't connect to the server
    pub fn is_blacklisted(&self, ip: &IpAddr) -> bool {
        let state = self.state.read();
        state.blacklisted_ips.contains(ip)
    }

    /// Returns the ban reason for `ip`, or `None` if the IP is not in the ban list.
    pub fn get_banned_reason(&self, ip: &IpAddr) -> Option<TextComponent> {
        let state = self.state.read();
        state
            .banned_ips_config_all
            .iter()
            .find(|b| b.ip == *ip)
            .map(|b| {
                MULTIPLAYER_DISCONNECT_BANNED_IP_REASON
                    .message([b.reason.clone()])
                    .component()
                    .add_child(MULTIPLAYER_DISCONNECT_BANNED_IP_EXPIRATION.message([
                        b.expires.map_or_else(
                            || "Never".to_string(),
                            |date| {
                                // Use the Local server DateTime instead of UTC (but precise in UTC)
                                let local_date: DateTime<Local> = date.into();
                                local_date.format("%Y-%m-%d %H:%M UTC%Z").to_string()
                            },
                        ),
                    ]))
            })
    }

    /// Persists the merged ban + blacklist file to `config/ip-bans.toml`.
    ///
    /// Called automatically from `SteelServer`'s `Drop` impl on clean shutdown.
    /// The whitelist is not written here — it lives in the server config.
    ///
    /// # Panics
    /// Panics if serialization or file write fails.
    pub fn save_config(&self) {
        fs::create_dir_all("config").expect("Failed to create config dir");
        let state = self.state.read();
        let bans_file = IpBansFile {
            ip_banned: state.banned_ips_config_all.clone(),
            blacklisted: state.blacklisted_ips.iter().copied().collect(),
        };
        let toml_out = toml::to_string_pretty(&bans_file).expect("Failed to serialize bans file");
        fs::write(BANS_PATH, &toml_out).expect("Failed to write config/ip-bans.toml");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ban(ip: &str, expires: Option<DateTime<Utc>>) -> BannedIP {
        BannedIP {
            ip: ip.parse().unwrap(),
            created: Utc.with_ymd_and_hms(2026, 4, 29, 22, 5, 51).unwrap(),
            source: "Server".into(),
            expires,
            reason: TextComponent::plain("Banned by an operator."),
        }
    }

    #[test]
    fn ip_bans_toml_roundtrips_with_finite_and_forever_expiry() {
        let finite = Utc.with_ymd_and_hms(2026, 5, 1, 22, 5, 51).unwrap();
        let original = IpBansFile {
            ip_banned: vec![
                make_ban("127.0.0.2", Some(finite)),
                make_ban("127.0.0.3", None),
            ],
            blacklisted: vec!["127.0.0.4".parse().unwrap()],
        };

        let serialized = toml::to_string_pretty(&original).unwrap();
        let reparsed: IpBansFile = toml::from_str(&serialized).unwrap();

        assert_eq!(reparsed.ip_banned.len(), 2);
        assert_eq!(reparsed.ip_banned[0].ip, original.ip_banned[0].ip);
        assert_eq!(reparsed.ip_banned[0].created, original.ip_banned[0].created);
        assert_eq!(reparsed.ip_banned[0].expires, Some(finite));
        assert_eq!(reparsed.ip_banned[1].expires, None);
        assert_eq!(reparsed.blacklisted, original.blacklisted);
    }
}
