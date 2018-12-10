//! A server that tells clients what time it is and where they are in the world.

#![deny(missing_docs)]

use actix_web::{App, HttpRequest, HttpResponse, http};
use chrono::{DateTime, Utc};
use listenfd::ListenFd;
use maxminddb::{self, geoip2, MaxMindDBError};
use serde::Serializer;
use serde_derive::Serialize;
use std::{env, fmt, net::IpAddr, process, path};
use futures::Future;

struct State {
    geoip: actix::Addr<GeoIpActor>,
}

fn main() {
    // Rust doesn't have a ctrl-c handler itself, so when running as
    // PID 1 in Docker it doesn't respond to SIGINT. This prevents
    // ctrl-c from stopping a docker container running this
    // program. Handle SIGINT (aka ctrl-c) to fix this problem.
    ctrlc::set_handler(move || {
        process::exit(0);
    })
    .expect("error setting ctrl-c handler");

    let sys = actix::System::new("classify-client");

    let geoip = actix::SyncArbiter::start(1, || {
        let geoip_path: path::PathBuf = "./GeoLite2-Country.mmdb".into();
        let reader = maxminddb::Reader::open(&geoip_path).expect(&format!("Could not open geoip database at {:?}", geoip_path));
        GeoIpActor { reader }
    });

    let server = actix_web::server::new(move || {
        App::with_state(State { geoip: geoip.clone() })
            .resource("/", |r| r.get().f(index))
    });

    // Re-use a passed file descriptor, or create a new one to listen on.
    let mut listenfd = ListenFd::from_env();
    let server = if let Some(listener) = listenfd.take_tcp_listener(0).expect("Could not get TCP listener") {
        println!("started server on re-used file descriptor");
        server.listen(listener)
    } else {
        let host = env::var("HOST").unwrap_or("127.0.0.1".to_string());
        let port = env::var("PORT").unwrap_or("8080".to_string());
        let addr = format!("{}:{}", host, port);
        println!("started server on https://{}:{}", host, port);
        server
            .bind(&addr)
            .expect(&format!("Couldn't listen on {}", &addr))
    };

    server.start();
    sys.run();
}

impl From<MaxMindDBError> for ClassifyError {
    fn from(error: MaxMindDBError) -> Self {
        match error {
            MaxMindDBError::AddressNotFoundError(msg) => ClassifyError { message: format!("AddressNotFound: {}", msg) },
            MaxMindDBError::InvalidDatabaseError(msg) => ClassifyError { message: format!("InvalidDatabaseError: {}", msg) },
            MaxMindDBError::IoError(msg) => ClassifyError { message: format!("IoError: {}", msg) },
            MaxMindDBError::MapError(msg) => ClassifyError { message: format!("MapError: {}", msg) },
            MaxMindDBError::DecodingError(msg) => ClassifyError { message: format!("DecodingError: {}", msg) },
        }
    }
}

struct GeoIpActor {
    reader: maxminddb::OwnedReader<'static>,
}

impl<'a> actix::Actor for GeoIpActor {
    type Context = actix::SyncContext<Self>;
}

impl actix::Handler<CountryForIp> for GeoIpActor {
    type Result = Result<Option<geoip2::Country>, ClassifyError>;

    fn handle(&mut self, msg: CountryForIp, _: &mut Self::Context) -> Self::Result {
        self.reader.lookup(msg.ip)
            .or_else(|err| {
                match err {
                    MaxMindDBError::AddressNotFoundError(_) => Ok(None),
                    _ => Err(err),
                }
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

#[derive(Debug, Serialize)]
struct ClassifyError {
    message: String,
}

impl ClassifyError {
    fn from<S: fmt::Display, E: fmt::Display>(source: S, err: E) -> Self {
        ClassifyError {
            message: format!("{}: {}", source, err),
        }
    }
}

// Use default implementation of Error
impl std::error::Error for ClassifyError {}

impl fmt::Display for ClassifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{:?}", self)?;
        Ok(())
    }
}

impl actix_web::error::ResponseError for ClassifyError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().json(self)
    }
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

fn index(req: &HttpRequest<State>) -> Box<dyn Future<Item=HttpResponse, Error=ClassifyError>> {
    let ip_res: Result<IpAddr, ClassifyError> = req.connection_info().remote()
        .ok_or(ClassifyError { message: "no ip".to_string() })
        .and_then(|remote| {
            remote.parse()
                .map_err(|err| ClassifyError::from(format!("IP ParseError for remote '{}'", remote), err))
        });
    let ip: IpAddr = match ip_res {
        Ok(v) => v,
        Err(err) => {
            return Box::new(futures::future::err(err));
        }
    };

    Box::new(req.state().geoip.send(CountryForIp { ip })
        .and_then(move |country| {
            let mut classification = ClientClassification::default();
            match country {
                Ok(country) => {
                    classification.country = country.clone();
                    Ok(
                        HttpResponse::Ok()
                        .header(http::header::CACHE_CONTROL, "max-age=0, no-cache, no-store, must-revalidate")
                        .json(classification)
                    )
                },
                Err(err) => {
                    Ok(
                        HttpResponse::InternalServerError().body(format!("{}", err))
                    )
                }
            }
        })
        .map_err(|err| ClassifyError { message: format!("Future failure: {}", err) })
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::StatusCode, test};
    use std::default::Default;

    #[test]
    fn test_index() {
        let req = test::TestRequest::default();
        let res = req.run(&index).unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
