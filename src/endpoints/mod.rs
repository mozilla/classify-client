pub mod classify;
pub mod debug;
pub mod dockerflow;

use std::default::Default;

use crate::{geoip::GeoIpActor, settings::Settings, APP_NAME};

#[derive(Clone)]
pub struct EndpointState {
    pub geoip: actix::Addr<GeoIpActor>,
    pub settings: Settings,
    pub log: slog::Logger,
    pub metrics: cadence::StatsdClient,
}

impl Default for EndpointState {
    fn default() -> Self {
        EndpointState {
            settings: Settings::default(),
            geoip: actix::SyncArbiter::start(1, GeoIpActor::default),
            log: slog::Logger::root(slog::Discard, slog::o!()).new(slog::o!()),
            metrics: cadence::StatsdClient::from_sink(APP_NAME, cadence::NopMetricSink),
        }
    }
}
