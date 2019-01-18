use cadence::{prelude::*, StatsdClient};
use maxminddb::{self, geoip2, MaxMindDBError};
use std::{net::IpAddr, path::PathBuf};

use crate::{errors::ClassifyError, settings::Settings};

pub fn get_arbiter(settings: &Settings, metrics: StatsdClient) -> actix::Addr<GeoIpActor> {
    let path = settings.geoip_db_path.clone();
    actix::SyncArbiter::start(1, move || {
        GeoIpActor::builder()
            .path(&path)
            .metrics(metrics.clone())
            .build()
            .unwrap_or_else(|err| {
                panic!(format!(
                    "Could not open geoip database at {:?}: {}",
                    path, err
                ))
            })
    })
}

pub struct GeoIpActor {
    reader: Option<maxminddb::OwnedReader<'static>>,
    metrics: StatsdClient,
}

impl Default for GeoIpActor {
    fn default() -> Self {
        Self::builder().build().unwrap()
    }
}

impl GeoIpActor {
    pub fn builder() -> GeoIpActorBuilder {
        GeoIpActorBuilder::default()
    }
}

#[derive(Default)]
pub struct GeoIpActorBuilder {
    path: Option<PathBuf>,
    metrics: Option<StatsdClient>,
}

impl GeoIpActorBuilder {
    pub fn path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn metrics(mut self, metrics: StatsdClient) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub fn build(self) -> Result<GeoIpActor, ClassifyError> {
        let reader = match self.path {
            Some(path) => Some(maxminddb::Reader::open(path)?),
            None => None,
        };

        let metrics: StatsdClient = self
            .metrics
            .unwrap_or_else(|| StatsdClient::from_sink("default", cadence::NopMetricSink));

        Ok(GeoIpActor { reader, metrics })
    }
}

impl<'a> actix::Actor for GeoIpActor {
    type Context = actix::SyncContext<Self>;
}

impl GeoIpActor {
    fn locate(&self, ip: IpAddr) -> Result<Option<geoip2::Country>, ClassifyError> {
        self.reader
            .as_ref()
            .ok_or_else(|| ClassifyError::new("No geoip database available"))?
            .lookup(ip)
            .map(|country_info: Option<geoip2::Country>| {
                // Send a metrics ping about the geolocation result
                let iso_code = country_info
                    .clone()
                    .and_then(|country_info| country_info.country)
                    .and_then(|country| country.iso_code);
                self.metrics
                    .incr_with_tags("location")
                    .with_tag("country", &iso_code.unwrap_or_else(|| "unknown".to_owned()))
                    .send();
                country_info
            })
            .or_else(|err| match err {
                MaxMindDBError::AddressNotFoundError(_) => {
                    self.metrics
                        .incr_with_tags("location")
                        .with_tag("country", "unknown")
                        .send();
                    Ok(None)
                }
                _ => Err(err),
            })
            .map_err(|err| err.into())
    }
}

impl actix::Handler<CountryForIp> for GeoIpActor {
    type Result = Result<Option<geoip2::Country>, ClassifyError>;

    fn handle(&mut self, msg: CountryForIp, _: &mut Self::Context) -> Self::Result {
        self.locate(msg.ip)
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

#[cfg(test)]
mod tests {
    use crate::metrics::tests::TestMetricSink;
    use cadence::StatsdClient;
    use std::{
        ops::Deref,
        sync::{Arc, Mutex},
    };

    #[test]
    fn test_geoip_works() -> Result<(), Box<dyn std::error::Error>> {
        let geoip = super::GeoIpActor::builder()
            .path("./GeoLite2-Country.mmdb")
            .build()?;

        let ip = "1.2.3.4".parse()?;
        let rv = geoip.locate(ip).unwrap().unwrap();
        assert_eq!(rv.country.unwrap().iso_code.unwrap(), "US");
        Ok(())
    }

    #[test]
    fn test_geoip_sends_metrics() -> Result<(), Box<dyn std::error::Error>> {
        let log = Arc::new(Mutex::new(Vec::new()));
        let metrics = StatsdClient::from_sink("test", TestMetricSink { log: log.clone() });
        let geoip = super::GeoIpActor::builder()
            .path("./GeoLite2-Country.mmdb")
            .metrics(metrics)
            .build()?;

        geoip.locate("1.2.3.4".parse()?)?;
        geoip.locate("127.0.0.1".parse()?)?;

        assert_eq!(
            *log.lock().unwrap().deref(),
            vec![
                "test.location:1|c|#country:US",
                "test.location:1|c|#country:unknown",
            ]
        );

        Ok(())
    }
}
