use config::{Config, ConfigError, Environment};
use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub geoip_db_path: PathBuf,
    pub human_logs: bool,
    pub version_file: PathBuf,
    pub sentry_dsn: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "[::]".to_owned(),
            port: 8080,
            geoip_db_path: "./GeoLite2-Country.mmdb".into(),
            human_logs: false,
            version_file: "./version.json".into(),
            sentry_dsn: "".into(),
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Config::new();

        let defaults = Config::try_from(&Settings::default())?;
        config.merge(defaults)?;

        let env = Environment::new();
        config.merge(env)?;

        config.try_into()
    }
}
