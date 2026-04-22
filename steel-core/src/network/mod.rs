use chrono::{DateTime, Local};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
mod local_datetime_format {
    use chrono::{DateTime, Local};
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &str = "%Y-%m-%d %H:%M:%S %z";

    pub fn serialize<S>(dt: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", dt.format(FORMAT));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // Parse as a DateTime with a fixed offset, then convert to Local.
        DateTime::parse_from_str(&s, FORMAT)
            .map(|dt| dt.with_timezone(&Local))
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct BannedIP {
    pub ip: String,
    #[serde(with = "local_datetime_format")]
    pub created: DateTime<Local>,
    pub source: String,
    #[serde(with = "local_datetime_format")]
    pub expires: DateTime<Local>,
    pub reason: String,
}

struct IPManager {
    banned_ips_config_all: Vec<BannedIP>,
    banned_ips_config_valid: Vec<BannedIP>,
    banned_ips: FxHashSet<String>,
    white_list_ips: FxHashSet<String>,
    // less cpu cycles then calling vec.len() every time in loop
    has_whitelist: bool,
}

impl IPManager {
    fn new() -> Self {
        Self {
            banned_ips: FxHashSet::default(),
            banned_ips_config_all: Vec::new(),
            banned_ips_config_valid: Vec::new(),
            white_list_ips: FxHashSet::default(),
            has_whitelist: false,
        }
    }
    pub fn ban_ip(&mut self, ip: String, source: String, reason: String, expires: DateTime<Local>) {
        self.banned_ips_config_all.push(BannedIP {
            ip,
            created: Local::now(),
            source,
            expires,
            reason,
        });
    }
    pub fn white_list_ip(&mut self, ip: String) {
        self.white_list_ips.insert(ip);
    }

    pub fn load_banned_ips(&mut self) {
        todo!()
    }

    pub fn load_whitelisted_ips(&mut self) {
        todo!()
    }

    pub fn can_join(&self) -> bool {
        false
    }

    pub fn save_config(&self) -> bool {
        true
    }
}
