pub mod classify;
pub mod debug;
pub mod dockerflow;
pub mod canned;
pub mod country;
use crate::{geoip::GeoIp, APP_NAME};
use std::{default::Default, path::PathBuf, sync::Arc};

#[derive(Clone, Debug)]
pub struct EndpointState {
    pub geoip: Arc<GeoIp>,
    pub trusted_proxies: Vec<ipnet::IpNet>,
    pub log: slog::Logger,
    pub metrics: Arc<cadence::StatsdClient>,
    pub version_file: PathBuf,
}

impl Default for EndpointState {
    fn default() -> Self {
        EndpointState {
            trusted_proxies: Vec::default(),
            geoip: Arc::new(GeoIp::default()),
            log: slog::Logger::root(slog::Discard, slog::o!()),
            metrics: Arc::new(cadence::StatsdClient::from_sink(
                APP_NAME,
                cadence::NopMetricSink,
            )),
            version_file: "./version.json".into(),
        }
    }
}
