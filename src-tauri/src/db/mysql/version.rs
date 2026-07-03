/// Parsed MySQL server version
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MySqlVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub is_mariadb: bool,
    pub raw: String,
    pub display: String,
}

/// Behavioral group used for version-specific query routing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MySqlVersionGroup {
    Legacy,      // < 5.7 or MariaDB
    Mysql57,     // 5.7.x
    Mysql80,     // 8.0.x – 8.3.x
    Mysql84Plus, // 8.4+
}

impl MySqlVersion {
    pub fn parse(raw: &str) -> Self {
        let is_mariadb = raw.to_lowercase().contains("mariadb");
        let version_str = raw.split('-').next().unwrap_or("0.0.0");
        let parts: Vec<u32> = version_str
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();

        let major = parts.get(0).cloned().unwrap_or(0);
        let minor = parts.get(1).cloned().unwrap_or(0);
        let patch = parts.get(2).cloned().unwrap_or(0);

        Self {
            major,
            minor,
            patch,
            is_mariadb,
            raw: raw.to_string(),
            display: format!("{}.{}.{}", major, minor, patch),
        }
    }

    pub fn group(&self) -> MySqlVersionGroup {
        if self.is_mariadb || self.major < 5 || (self.major == 5 && self.minor < 7) {
            return MySqlVersionGroup::Legacy;
        }
        if self.major == 5 && self.minor == 7 {
            return MySqlVersionGroup::Mysql57;
        }
        if self.major == 8 && self.minor < 4 {
            return MySqlVersionGroup::Mysql80;
        }
        if self.major >= 8 {
            return MySqlVersionGroup::Mysql84Plus;
        }
        MySqlVersionGroup::Legacy
    }
}
