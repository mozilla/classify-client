use crate::{endpoints::EndpointState, errors::ClassifyError, utils::RequestClientIp};
use actix_web::{http, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use maxminddb::{self, geoip2};
use serde::Serializer;
use serde_derive::Serialize;

#[derive(Serialize)]
struct ClientClassification<'a> {
    request_time: DateTime<Utc>,

    #[serde(serialize_with = "country_iso_code")]
    country: Option<geoip2::Country<'a>>,
}

fn country_iso_code<S: Serializer>(
    country_info: &Option<geoip2::Country>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let iso_code: Option<&str> = country_info
        .clone()
        .and_then(|country_info| country_info.country)
        .and_then(|country| country.iso_code);

    match iso_code {
        Some(code) => serializer.serialize_str(code),
        None => serializer.serialize_none(),
    }
}

impl<'a> Default for ClientClassification<'a> {
    fn default() -> Self {
        Self {
            request_time: Utc::now(),
            country: None,
        }
    }
}

pub async fn classify_client(req: HttpRequest) -> Result<HttpResponse, ClassifyError> {
    req.app_data::<EndpointState>()
        .expect("Could not get app state")
        .geoip
        .locate(req.client_ip()?)
        .map(move |country| {
            let mut response = HttpResponse::Ok();
            response.append_header((
                http::header::CACHE_CONTROL,
                "max-age=0, no-cache, no-store, must-revalidate",
            ));
            response.json(ClientClassification {
                country,
                ..Default::default()
            })
        })
        .map_err(|err| ClassifyError::from_source("Future failure", err))
}

#[cfg(test)]
mod tests {
    use crate::{endpoints::EndpointState, geoip::GeoIp};
    use actix_web::{
        http,
        test::{self, TestRequest},
        web, App,
    };
    use chrono::DateTime;
    use maxminddb::geoip2;
    use serde_json::{json, Value};
    use std::{collections::HashSet, sync::Arc};

    #[actix_rt::test]
    async fn test_classification_serialization() {
        let mut classification = super::ClientClassification::default();

        let value = serde_json::to_value(&classification).unwrap();
        assert_eq!(*value.get("country").unwrap(), Value::Null);

        classification.country = Some(geoip2::Country {
            country: Some(geoip2::country::Country {
                geoname_id: None,
                iso_code: Some("US"),
                names: None,
                is_in_european_union: None,
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

    #[actix_rt::test]
    async fn test_classify_endpoint() -> Result<(), Box<dyn std::error::Error>> {
        let state = EndpointState {
            geoip: Arc::new(
                GeoIp::builder()
                    .path("./GeoLite2-Country.mmdb")
                    .build()
                    .unwrap(),
            ),
            trusted_proxies: vec!["127.0.0.1/32".parse().unwrap()],
            ..EndpointState::default()
        };
        let service = test::init_service(
            App::new()
                .app_data(state)
                .route("/", web::get().to(super::classify_client)),
        )
        .await;

        let request = TestRequest::get()
            .insert_header(("x-forwarded-for", "7.7.7.7"))
            .to_request();
        let value: serde_json::Value = test::call_and_read_body_json(&service, request).await;
        assert_eq!(
            *value.get("country").unwrap(),
            json!("US"),
            "Geoip should resolve a known IP"
        );

        let timestamp = value.get("request_time").unwrap().as_str().unwrap();
        // RFC 3339 is a stricter form of the ISO 8601 timestamp format.
        let parse_result = DateTime::parse_from_rfc3339(timestamp);
        assert!(
            parse_result.is_ok(),
            "request time should be a valid timestamp"
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn test_classify_endpoint_has_correct_cache_headers() {
        let service = test::init_service(
            App::new()
                .app_data(EndpointState {
                    geoip: Arc::new(
                        GeoIp::builder()
                            .path("./GeoLite2-Country.mmdb")
                            .build()
                            .unwrap(),
                    ),
                    ..EndpointState::default()
                })
                .route("/", web::get().to(super::classify_client)),
        )
        .await;

        let request = TestRequest::get()
            .insert_header(("x-forwarded-for", "1.2.3.4"))
            .to_request();
        let response = test::call_service(&service, request).await;

        assert_eq!(response.status(), http::StatusCode::OK);
        let headers = response.headers();
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
