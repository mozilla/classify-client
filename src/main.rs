#![deny(clippy::all)]

//! A server that tells clients what time it is and where they are in the world.

#![deny(missing_docs)]

use actix_web::{http, App, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use futures::Future;
use maxminddb::{self, geoip2, MaxMindDBError};
use serde::Serializer;
use serde_derive::Serialize;
use std::{env, net::IpAddr, path::PathBuf, process};

use crate::errors::ClassifyError;

mod errors;

struct State {
    geoip: actix::Addr<GeoIpActor>,
}

fn main() {
    // Rust doesn't have a ctrl-c handler itself, so when running as
    // PID 1 in Docker it doesn't respond to SIGINT. This prevents
    // ctrl-c from stopping a docker container running this
    // program. Handle SIGINT (aka ctrl-c) to fix this problem.
    ctrlc::set_handler(move || process::exit(0)).expect("error setting ctrl-c handler");

    let sys = actix::System::new("classify-client");

    let geoip = actix::SyncArbiter::start(1, || {
        let geoip_path = "./GeoLite2-Country.mmdb";
        GeoIpActor::from_path(&geoip_path).unwrap_or_else(|err| {
            panic!(format!(
                "Could not open geoip database at {:?}: {}",
                geoip_path, err
            ))
        })
    });

    let host = env::var("HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("{}:{}", host, port);

    let server = actix_web::server::new(move || {
        App::with_state(State {
            geoip: geoip.clone(),
        })
        .resource("/", |r| r.get().f(index))
    })
    .bind(&addr)
    .unwrap_or_else(|err| panic!(format!("Couldn't listen on {}: {}", &addr, err)));

    server.start();
    println!("started server on https://{}:{}", host, port);
    sys.run();
}

struct GeoIpActor {
    reader: maxminddb::OwnedReader<'static>,
}

impl GeoIpActor {
    fn from_path<P: Into<PathBuf>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
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

struct CountryForIp {
    ip: IpAddr,
}

impl actix::Message for CountryForIp {
    type Result = Result<Option<geoip2::Country>, ClassifyError>;
}

#[derive(Serialize)]
struct ClientClassification {
    request_time: DateTime<Utc>,

    #[serde(serialize_with = "country_iso_code")]
    country: Option<geoip2::Country>,
}

fn country_iso_code<S: Serializer>(
    country_info: &Option<geoip2::Country>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let iso_code: Option<String> = country_info
        .clone()
        .and_then(|country_info| country_info.country)
        .and_then(|country| country.iso_code);

    match iso_code {
        Some(code) => serializer.serialize_str(&code),
        None => serializer.serialize_none(),
    }
}

impl Default for ClientClassification {
    fn default() -> Self {
        Self {
            request_time: Utc::now(),
            country: None,
        }
    }
}

/// Determine the IP address of the client making a request, based on network
/// information and headers.
fn get_client_ip<S>(request: &HttpRequest<S>) -> Result<IpAddr, ClassifyError> {
    // Actix has a method to do this, but it returns a string, and doesn't strip
    // off ports if present, so it is difficult to use.

    if let Some(x_forwarded_for) = request.headers().get("X-Forwarded-For") {
        let ips: Vec<_> = x_forwarded_for
            .to_str()?
            .split(',')
            .map(|ip| ip.trim())
            .collect();
        if ips.len() == 1 {
            return Ok(ips[0].parse()?);
        } else if ips.len() > 1 {
            // the last item is probably a google load balancer, strip that off, use the second-to-last item.
            return Ok(ips[ips.len() - 2].parse()?);
        }
        // 0 items is an empty header, and weird. fall back to peer address detection
    }

    // No headers were present, so use the peer address directly
    if let Some(peer_addr) = request.peer_addr() {
        return Ok(peer_addr.ip());
    }

    Err(ClassifyError::new("Could not determine IP"))
}

fn index(req: &HttpRequest<State>) -> Box<dyn Future<Item = HttpResponse, Error = ClassifyError>> {
    // TODO this is the sort of thing that the try operator (`?`) is supposed to
    // be for. Is it possible to use the try operator with `Box<dyn Future<_>>`?
    let ip = match get_client_ip(req) {
        Ok(v) => v,
        Err(err) => {
            return Box::new(futures::future::err(err));
        }
    };

    Box::new(
        req.state()
            .geoip
            .send(CountryForIp { ip })
            .and_then(move |country| {
                let mut classification = ClientClassification::default();
                match country {
                    Ok(country) => {
                        classification.country = country.clone();
                        Ok(HttpResponse::Ok()
                            .header(
                                http::header::CACHE_CONTROL,
                                "max-age=0, no-cache, no-store, must-revalidate",
                            )
                            .json(classification))
                    }
                    Err(err) => Ok(HttpResponse::InternalServerError().body(format!("{}", err))),
                }
            })
            .map_err(|err| ClassifyError::from_source("Future failure", err)),
    )
}
