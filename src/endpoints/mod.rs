pub mod classify;
pub mod debug;
pub mod dockerflow;

use std::{default::Default, path::PathBuf};

use crate::{geoip::GeoIpActor, APP_NAME};

#[derive(Clone, Debug)]
pub struct EndpointState {
    pub geoip: actix::Addr<GeoIpActor>,
    pub trusted_proxies: Vec<ipnet::IpNet>,
    pub log: slog::Logger,
    pub metrics: cadence::StatsdClient,
    pub version_file: PathBuf,
}

impl Default for EndpointState {
    fn default() -> Self {
        EndpointState {
            trusted_proxies: Vec::default(),
            geoip: actix::SyncArbiter::start(1, GeoIpActor::default),
            log: slog::Logger::root(slog::Discard, slog::o!()),
            metrics: cadence::StatsdClient::from_sink(APP_NAME, cadence::NopMetricSink),
            version_file: "./version.json".into(),
        }
    }
}
