use crate::errors::ClassifyError;
use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_geoip_db_path() -> PathBuf {
    "./GeoLite2-Country.mmdb".into()
}

fn default_host() -> String {
    "[::]".to_owned()
}

fn default_port() -> u16 {
    8000
}

fn default_version_file() -> PathBuf {
    "./version.json".into()
}

fn default_metrics_target() -> String {
    "localhost:8125".to_owned()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub debug: bool,

    #[serde(default = "default_geoip_db_path")]
    pub geoip_db_path: PathBuf,

    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default)]
    pub trusted_proxy_list: Vec<ipnet::IpNet>,

    #[serde(default)]
    pub human_logs: bool,

    #[serde(default = "default_version_file")]
    pub version_file: PathBuf,

    pub sentry_dsn: Option<String>,

    /// The host and port to send statsd metrics to. May be a hostname like
    /// "metrics.example.com:8125" or an ip like "127.0.0.1:8125". Port is
    /// required. Defaults to "localhost:8125".
    #[serde(default = "default_metrics_target")]
    pub metrics_target: String,
}

impl Default for Settings {
    fn default() -> Self {
        // Produce a default value by creating a mock empty environment, and
        // then asking envy to deserialize it. Since all settings have a default
        // value specified in the struct, this works and keeps everything in sync.
        let empty_env: Vec<(String, String)> = Vec::new();
        envy::from_iter(empty_env.into_iter()).unwrap()
    }
}

impl Settings {
    /// Load settings from the environment.
    pub fn load() -> Result<Self, ClassifyError> {
        let settings: Self = envy::from_env()?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use crate::settings::Settings;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();

        assert!(!settings.debug);
        assert_eq!(
            settings.geoip_db_path.to_str(),
            Some("./GeoLite2-Country.mmdb")
        );
        assert_eq!(settings.host, "[::]");
        assert_eq!(settings.port, 8000);
        assert_eq!(settings.trusted_proxy_list, Vec::new());
        assert!(!settings.human_logs);
        assert_eq!(settings.version_file.to_str(), Some("./version.json"));
        assert_eq!(settings.sentry_dsn, None);
        assert_eq!(settings.metrics_target, "localhost:8125");
    }

    #[test]
    fn test_override_via_env_vars() {
        env::set_var("DEBUG", "true");
        env::set_var("PORT", "8888");
        env::set_var("TRUSTED_PROXY_LIST", "2001:db8::/48,192.168.100.14/24");

        let settings = Settings::load().unwrap();

        assert!(settings.debug);
        assert_eq!(settings.port, 8888);
        assert_eq!(settings.trusted_proxy_list.len(), 2);
    }
}
