use actix_web::{http, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use futures::Future;
use maxminddb::{self, geoip2};
use serde::Serializer;
use serde_derive::Serialize;

use crate::{
    endpoints::EndpointState, errors::ClassifyError, geoip::CountryForIp, utils::RequestClientIp,
};

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

pub fn classify_client(
    req: &HttpRequest<EndpointState>,
) -> Box<dyn Future<Item = HttpResponse, Error = ClassifyError>> {
    // TODO this is the sort of thing that the try operator (`?`) is supposed to
    // be for. Is it possible to use the try operator with `Box<dyn Future<_>>`?
    let ip = match req.client_ip() {
        Ok(v) => v,
        Err(err) => {
            return Box::new(futures::future::err(err));
        }
    };

    Box::new(
        req.state()
            .geoip
            .send(CountryForIp::new(ip))
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
