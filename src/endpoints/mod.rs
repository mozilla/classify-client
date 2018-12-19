pub mod classify;
pub mod debug;
pub mod dockerflow;

use std::default::Default;

use crate::{geoip::GeoIpActor, settings::Settings};

#[derive(Clone)]
pub struct EndpointState {
    pub geoip: actix::Addr<GeoIpActor>,
    pub settings: Settings,
}

impl Default for EndpointState {
    fn default() -> Self {
        EndpointState {
            settings: Settings::default(),
            geoip: actix::SyncArbiter::start(1, GeoIpActor::default),
        }
    }
}
