use crate::errors::ClassifyError;
use cadence::{prelude::*, StatsdClient};
use maxminddb::{self, geoip2, MaxMindDBError};
use std::{fmt, net::IpAddr, path::PathBuf};

pub struct GeoIp {
    reader: Option<maxminddb::Reader<Vec<u8>>>,
    metrics: StatsdClient,
}

impl GeoIp {
    pub fn builder() -> GeoIpBuilder {
        GeoIpBuilder::default()
    }

    pub fn locate(&self, ip: IpAddr) -> Result<Option<geoip2::Country>, ClassifyError> {
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

impl Default for GeoIp {
    fn default() -> Self {
        GeoIp::builder().build().unwrap()
    }
}

// // maxminddb reader doesn't implement Debug, so we can't use #[derive(Debug)] on GeoIp.
impl fmt::Debug for GeoIp {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "GeoIpActor {{ reader: {}, metrics: {:?} }}",
            if self.reader.is_some() {
                "Some(...)"
            } else {
                "None"
            },
            self.metrics
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct GeoIpBuilder {
    path: Option<PathBuf>,
    metrics: Option<StatsdClient>,
}

impl GeoIpBuilder {
    pub fn path<P>(mut self, path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.path = Some(path.into());
        self
    }

    pub fn metrics(mut self, metrics: StatsdClient) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub fn build(self) -> Result<GeoIp, ClassifyError> {
        let reader = match self.path {
            Some(path) => Some(maxminddb::Reader::open_readfile(path)?),
            None => None,
        };
        let metrics = self
            .metrics
            .unwrap_or_else(|| StatsdClient::from_sink("default", cadence::NopMetricSink));
        Ok(GeoIp { reader, metrics })
    }
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
        let geoip = super::GeoIp::builder()
            .path("./GeoLite2-Country.mmdb")
            .build()?;

        let ip = "7.7.7.7".parse()?;
        let rv = geoip.locate(ip).unwrap().unwrap();
        assert_eq!(rv.country.unwrap().iso_code.unwrap(), "US");
        Ok(())
    }

    #[test]
    fn test_geoip_sends_metrics() -> Result<(), Box<dyn std::error::Error>> {
        let log = Arc::new(Mutex::new(Vec::new()));
        let metrics = StatsdClient::from_sink("test", TestMetricSink { log: log.clone() });
        let geoip = super::GeoIp::builder()
            .path("./GeoLite2-Country.mmdb")
            .metrics(metrics)
            .build()?;

        geoip.locate("7.7.7.7".parse()?)?;
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
