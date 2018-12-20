use config::{Config, Environment};
use ipnet::IpNet;
use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::errors::ClassifyError;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub debug: bool,
    pub geoip_db_path: PathBuf,
    pub host: String,
    pub port: u16,
    // TODO This should be Vec<ipnet::IpNet>, but the config crate seemingly
    // can't deal with Vecs of non primitives?
    pub trusted_proxy_list: Vec<String>,
    pub version_file: PathBuf,
    pub sentry_dsn: String,
}

impl Settings {
    /// Check that all settings are well-formed
    pub fn check(&self) -> Result<(), ClassifyError> {
        self.trusted_proxy_list()?;
        Ok(())
    }

    /// Get a list of trusted IP address ranges
    pub fn trusted_proxy_list(&self) -> Result<Vec<IpNet>, ClassifyError> {
        let ips: Result<Vec<IpNet>, _> = self
            .trusted_proxy_list
            .iter()
            .map(|s| {
                s.parse().map_err(|err| {
                    ClassifyError::from_source(format!("While parsing IP range {:?}", s), err)
                })
            })
            .collect();
        Ok(ips?)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            debug: false,
            geoip_db_path: "./GeoLite2-Country.mmdb".into(),
            host: "[::]".to_owned(),
            port: 8080,
            trusted_proxy_list: Vec::new(),
            version_file: "./version.json".into(),
            sentry_dsn: "".into(),
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self, ClassifyError> {
        let mut config = Config::new();

        let defaults = {
            let settings = Settings::default();
            let mut default_config = Config::try_from(&settings)?;
            // TODO this doesn't get pulled over from Settings::Default for some reason?
            default_config.set_default("trusted_proxy_list", settings.trusted_proxy_list)?;
            default_config
        };
        config.merge(defaults)?;

        let mut env = Config::new();
        env.merge(Environment::new())?;
        if let Ok(csv_ip_ranges) = env.get_str("trusted_proxy_list") {
            // Split the String into a Vec<String>
            env.set(
                "trusted_proxy_list",
                csv_ip_ranges
                    .split(',')
                    .map(|v| v.trim())
                    .collect::<Vec<_>>(),
            )?;
        }
        config.merge(env)?;

        let rv: Settings = config.try_into()?;

        // Test that all settings are well formed
        rv.check()?;

        Ok(rv)
    }
}
