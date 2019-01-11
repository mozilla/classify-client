use actix_web::{http, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use futures::Future;
use maxminddb::{self, geoip2};
use serde::Serializer;
use serde_derive::Serialize;
use serde_json::json;

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
                let mut response = HttpResponse::Ok();
                response.header(
                    http::header::CACHE_CONTROL,
                    "max-age=0, no-cache, no-store, must-revalidate",
                );

                let mut classification = ClientClassification::default();
                match country {
                    Ok(country) => {
                        classification.country = country.clone();
                        Ok(response.json(classification))
                    }
                    Err(err) => Ok(response
                        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                        .json(json!({ "error": format!("{}", err) }))),
                }
            })
            .map_err(|err| ClassifyError::from_source("Future failure", err)),
    )
}

#[cfg(test)]
mod tests {
    use crate::{endpoints::EndpointState, geoip::GeoIpActor, settings::Settings};
    use actix_web::{http, test, HttpMessage};
    use chrono::DateTime;
    use maxminddb::geoip2;
    use serde_json::{json, Value};
    use std::{collections::HashSet, path::PathBuf};

    #[test]
    fn test_classification_serialization() {
        let mut classification = super::ClientClassification::default();

        let value = serde_json::to_value(&classification).unwrap();
        assert_eq!(*value.get("country").unwrap(), Value::Null);

        classification.country = Some(geoip2::Country {
            country: Some(geoip2::model::Country {
                geoname_id: None,
                iso_code: Some("US".to_owned()),
                names: None,
            }),
            continent: None,
            registered_country: None,
            represented_country: None,
            traits: None,
        });

        let value = serde_json::to_value(&classification).unwrap();
        assert_eq!(
            *value.get("country").unwrap(),
            Value::String("US".to_owned())
        );
    }

    #[test]
    fn test_classify_endpoint() {
        let mut srv = test::TestServer::build_with_state(|| EndpointState {
            geoip: {
                let path: PathBuf = "./GeoLite2-Country.mmdb".into();
                actix::SyncArbiter::start(1, move || GeoIpActor::from_path(&path).unwrap())
            },
            settings: Settings {
                trusted_proxy_list: vec!["127.0.0.1/32".parse().unwrap()],
                ..Settings::default()
            },
            ..EndpointState::default()
        })
        .start(|app| app.handler(&super::classify_client));

        let req = srv
            .get()
            .header("x-forwarded-for", "1.2.3.4")
            .finish()
            .unwrap();
        let resp = srv.execute(req.send()).unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let value: serde_json::Value = srv.execute(resp.json()).unwrap();
        assert_eq!(
            *value.get("country").unwrap(),
            json!("US"),
            "Geoip should resolve a known IP"
        );

        let timestamp = value.get("request_time").unwrap().as_str().unwrap();
        // RFC 3339 is a stricter form of the ISO 8601 timestamp format.
        let parse_result = DateTime::parse_from_rfc3339(&timestamp);
        assert!(
            parse_result.is_ok(),
            "request time should be a valid timestamp"
        );
    }

    #[test]
    fn test_classify_endpoint_has_correct_cache_headers() {
        let mut srv = test::TestServer::build_with_state(EndpointState::default)
            .start(|app| app.handler(&super::classify_client));

        let req = srv.get().finish().unwrap();
        let resp = srv.execute(req.send()).unwrap();

        let headers = resp.headers();
        assert!(
            headers.contains_key(http::header::CACHE_CONTROL),
            "a cache control header should be set"
        );
        let cache_items: HashSet<_> = headers
            .get(http::header::CACHE_CONTROL)
            .unwrap()
            .to_str()
            .unwrap()
            .split(',')
            .map(|s| s.trim())
            .collect();
        let expected = vec!["max-age=0", "no-cache", "no-store", "must-revalidate"]
            .into_iter()
            .collect();
        assert_eq!(cache_items, expected);
    }
}
