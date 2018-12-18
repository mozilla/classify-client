use maxminddb::{self, geoip2, MaxMindDBError};
use std::{net::IpAddr, path::PathBuf};

use crate::errors::ClassifyError;

pub struct GeoIpActor {
    reader: maxminddb::OwnedReader<'static>,
}

impl GeoIpActor {
    pub fn from_path<P: Into<PathBuf>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.into();
        let reader = maxminddb::Reader::open(path)?;
        Ok(Self { reader })
    }
}

impl<'a> actix::Actor for GeoIpActor {
    type Context = actix::SyncContext<Self>;
}

impl actix::Handler<CountryForIp> for GeoIpActor {
    type Result = Result<Option<geoip2::Country>, ClassifyError>;

    fn handle(&mut self, msg: CountryForIp, _: &mut Self::Context) -> Self::Result {
        self.reader
            .lookup(msg.ip)
            .or_else(|err| match err {
                MaxMindDBError::AddressNotFoundError(_) => Ok(None),
                _ => Err(err),
            })
            .map_err(|err| err.into())
    }
}

pub struct CountryForIp {
    ip: IpAddr,
}

impl CountryForIp {
    pub fn new(ip: IpAddr) -> Self {
        Self { ip }
    }
}

impl actix::Message for CountryForIp {
    type Result = Result<Option<geoip2::Country>, ClassifyError>;
}
