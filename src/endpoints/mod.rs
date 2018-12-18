pub mod classify;
pub mod debug;
pub mod dockerflow;

use crate::{geoip::GeoIpActor, settings::Settings};

#[derive(Clone)]
pub struct EndpointState {
    pub geoip: actix::Addr<GeoIpActor>,
    pub settings: Settings,
}
