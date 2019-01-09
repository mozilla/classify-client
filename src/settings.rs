use serde_derive::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

use crate::errors::ClassifyError;

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
    pub trusted_proxy_list: HashSet<ipnet::IpNet>,

    #[serde(default)]
    pub human_logs: bool,

    #[serde(default = "default_version_file")]
    pub version_file: PathBuf,

    pub sentry_dsn: Option<String>,
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
