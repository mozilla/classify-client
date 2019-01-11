pub mod classify;
pub mod debug;
pub mod dockerflow;

use std::default::Default;

use crate::{geoip::GeoIpActor, logging::MozLogger, settings::Settings};

#[derive(Clone)]
pub struct EndpointState {
    pub geoip: actix::Addr<GeoIpActor>,
    pub settings: Settings,
    pub log: MozLogger,
    pub metrics: cadence::StatsdClient,
}

impl Default for EndpointState {
    fn default() -> Self {
        EndpointState {
            settings: Settings::default(),
            geoip: actix::SyncArbiter::start(1, GeoIpActor::default),
            log: MozLogger::default(),
            metrics: cadence::StatsdClient::from_sink("classify-client", cadence::NopMetricSink),
        }
    }
}
