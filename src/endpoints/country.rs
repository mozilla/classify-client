use crate::{endpoints::EndpointState, errors::ClassifyError, utils::RequestClientIp};
use actix_web::{http, web::Data, web::Query, HttpRequest, HttpResponse};
use cadence::prelude::*;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize)]
struct CountryResponse<'a> {
    country_code: &'a str,
    country_name: &'a str,
}

#[derive(Serialize)]
struct CountryNotFoundResponse<'a> {
    errors: &'a [CountryNotFoundError<'a>],
    code: i16,
    message: &'a str,
}

#[derive(Serialize)]
struct CountryNotFoundError<'a> {
    domain: &'a str,
    reason: &'a str,
    message: &'a str,
}

static COUNTRY_NOT_FOUND_RESPONSE: CountryNotFoundResponse = CountryNotFoundResponse {
    code: 404,
    message: "Not found",
    errors: &[CountryNotFoundError {
        domain: "geolocation",
        reason: "notFound",
        message: "Not found",
    }],
};

static DOWNSTREAM_KEY: Lazy<regex::Regex> =
    Lazy::new(|| Regex::new(r"^firefox-downstream-\w{1,40}$").unwrap());

#[derive(Deserialize, Debug)]
pub struct Params {
    key: String,
}

pub async fn get_country(
    req: HttpRequest,
    state: Data<EndpointState>,
) -> Result<HttpResponse, ClassifyError> {
    let metrics = &state.metrics;

    // check provided API Key
    match Query::<Params>::from_query(req.query_string()) {
        Ok(req_query) => {
            // check for downstream firefox regex pattern, see readme for details
            if !DOWNSTREAM_KEY.is_match(&req_query.key) {
                // if that misses, check list of known API keys
                if !state.api_keys_hashset.contains(&req_query.key) {
                    metrics
                        .incr_with_tags("country")
                        .with_tag("api_key", "invalid-key")
                        .send();
                    return Ok(HttpResponse::Unauthorized().body("Wrong key"));
                }
            }

            metrics
                .incr_with_tags("country")
                .with_tag("api_key", &req_query.key)
                .send();
        }
        _ => {
            return Ok(HttpResponse::Unauthorized().body("Wrong key"));
        }
    }

    // return country if we can identify it based on IP address
    return state
        .geoip
        .locate(req.client_ip()?)
        .map(move |location| {
            let country_opt = match location {
                Some(x) => x.country,
                None => None,
            };

            if country_opt.is_none() {
                let mut response = HttpResponse::NotFound();
                metrics.incr_with_tags("country_miss").send();
                return response.json(&COUNTRY_NOT_FOUND_RESPONSE);
            }

            let mut response = HttpResponse::Ok();
            response.append_header((
                http::header::CACHE_CONTROL,
                "max-age=0, no-cache, no-store, must-revalidate",
            ));

            metrics.incr_with_tags("country_hit").send();

            let country = country_opt.unwrap();
            response.json(CountryResponse {
                country_code: match country.iso_code {
                    Some(x) => x,
                    None => "",
                },
                country_name: match country.names {
                    Some(x) => x["en"],
                    None => "",
                },
            })
        })
        .map_err(|err| ClassifyError::from_source("Future failure", err));
}

#[cfg(test)]
mod tests {
    use crate::{endpoints::EndpointState, geoip::GeoIp, metrics::tests::TestMetricSink};
    use actix_web::{
        test::{self, TestRequest},
        web::{self, Data},
        App,
    };
    use cadence::StatsdClient;
    use serde_json::{self, json};
    use std::{
        collections::HashSet,
        ops::Deref,
        sync::{Arc, Mutex},
    };

    #[actix_rt::test]
    async fn test_country_endpoint() -> Result<(), Box<dyn std::error::Error>> {
        let log = Arc::new(Mutex::new(Vec::new()));
        let metrics = Arc::new(StatsdClient::from_sink(
            "test",
            TestMetricSink { log: log.clone() },
        ));
        let mut api_keys_hashset = HashSet::new();
        api_keys_hashset.insert("testkey".to_string());

        let state = EndpointState {
            api_keys_hashset,
            geoip: Arc::new(
                GeoIp::builder()
                    .path("./GeoLite2-Country.mmdb")
                    .build()
                    .unwrap(),
            ),
            trusted_proxies: vec!["127.0.0.1/32".parse().unwrap()],
            metrics,
            ..EndpointState::default()
        };
        let service = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .route("/", web::get().to(super::get_country)),
        )
        .await;

        let missing_key_request = TestRequest::get()
            .param("key", "testkey")
            .insert_header(("x-forwarded-for", "127.0.0.2"))
            .to_request();
        let missing_key_response = test::call_service(&service, missing_key_request).await;
        assert_eq!(
            missing_key_response.status(),
            401,
            "Geoip should return 401 http status for an API key miss"
        );

        let miss_request = TestRequest::get()
            .uri("/?key=testkey")
            .insert_header(("x-forwarded-for", "127.0.0.2"))
            .to_request();
        let miss_response = test::call_service(&service, miss_request).await;
        assert_eq!(
            miss_response.status(),
            404,
            "Geoip should return 404 http status for an unknown IP"
        );
        let miss_value: serde_json::Value = test::read_body_json(miss_response).await;
        assert_eq!(
            *miss_value.get("code").unwrap(),
            json!(404),
            "Geoip should return 404 for an unknown IP"
        );
        assert_eq!(
            *miss_value.get("message").unwrap(),
            json!("Not found"),
            "Geoip should return 404 for an unknown IP"
        );

        let hit_request = TestRequest::get()
            .uri("/?key=testkey")
            .insert_header(("x-forwarded-for", "7.7.7.7"))
            .to_request();
        let hit_value: serde_json::Value =
            test::call_and_read_body_json(&service, hit_request).await;
        assert_eq!(
            *hit_value.get("country_code").unwrap(),
            json!("US"),
            "Geoip should resolve a country code for known IP"
        );
        assert_eq!(
            *hit_value.get("country_name").unwrap(),
            json!("United States"),
            "Geoip should resolve a country name for known IP"
        );

        // check a valid downstream API key using the \w{1-40} pattern
        let downstream_key_request = TestRequest::get()
            .uri("/?key=firefox-downstream-foo_bar")
            .insert_header(("x-forwarded-for", "7.7.7.7"))
            .to_request();
        let downstream_key_value: serde_json::Value =
            test::call_and_read_body_json(&service, downstream_key_request).await;
        assert_eq!(
            *downstream_key_value.get("country_code").unwrap(),
            json!("US"),
            "Geoip should resolve a country code for known IP"
        );
        assert_eq!(
            *downstream_key_value.get("country_name").unwrap(),
            json!("United States"),
            "Geoip should resolve a country name for known IP"
        );

        // check an imvalid downstream API key using the \w{1-40} pattern
        let downstream_key_invalid = TestRequest::get()
            .uri("/?key=firefox-downstream-foo-bar") // dashes won't match \w
            .insert_header(("x-forwarded-for", "7.7.7.7"))
            .to_request();
        let downstream_key_invalid_response =
            test::call_service(&service, downstream_key_invalid).await;
        assert_eq!(
            downstream_key_invalid_response.status(),
            401,
            "Geoip should return 401 http status for an API key miss"
        );

        // check that we have api key metrics
        assert_eq!(
            *log.lock().unwrap().deref(),
            vec![
                "test.country:1|c|#api_key:testkey",
                "test.country_miss:1|c",
                "test.country:1|c|#api_key:testkey",
                "test.country_hit:1|c",
                "test.country:1|c|#api_key:firefox-downstream-foo_bar",
                "test.country_hit:1|c",
                "test.country:1|c|#api_key:invalid-key",
            ]
        );

        Ok(())
    }
}
