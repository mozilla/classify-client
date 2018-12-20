pub mod classify;
pub mod dockerflow;

use crate::{geoip::GeoIpActor, logging::MozLogger, settings::Settings};

#[derive(Clone)]
pub struct EndpointState {
    pub geoip: actix::Addr<GeoIpActor>,
    pub settings: Settings,
    pub log: MozLogger,
}
