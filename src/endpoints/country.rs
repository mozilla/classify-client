use crate::{endpoints::EndpointState, errors::ClassifyError, utils::RequestClientIp};
use actix_web::{http, web::Data, HttpRequest, HttpResponse};
use serde_derive::Serialize;

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

pub async fn get_country(
    req: HttpRequest,
    state: Data<EndpointState>,
) -> Result<HttpResponse, ClassifyError> {
    return state
        .geoip
        .locate(req.client_ip()?)
        .map(move |location| {
            let country_opt = match location {
                Some(x) => x.country,
                None => None
            };

            if country_opt.is_none() {
                let mut response = HttpResponse::NotFound();
                return response.json(&COUNTRY_NOT_FOUND_RESPONSE);
            }

            let mut response = HttpResponse::Ok();
            response.append_header((
                http::header::CACHE_CONTROL,
                "max-age=0, no-cache, no-store, must-revalidate",
            ));

            let country = country_opt.unwrap();
            response.json(CountryResponse {
                country_code: match country.iso_code {
                    Some(x) => x,
                    None => ""
                },
                country_name: match country.names {
                    Some(x) => x["en"],
                    None => ""
                },
            })
        })
        .map_err(|err| ClassifyError::from_source("Future failure", err));
}

#[cfg(test)]
mod tests {
    use crate::{endpoints::EndpointState, geoip::GeoIp};
    use actix_web::{
        test::{self, TestRequest},
        web::{self, Data},
        App,
    };
    use serde_json::{self, json};
    use std::sync::Arc;

    #[actix_rt::test]
    async fn test_country_endpoint() -> Result<(), Box<dyn std::error::Error>> {
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
                .app_data(Data::new(state))
                .route("/", web::get().to(super::get_country)),
        )
        .await;

        let miss_request = TestRequest::get()
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

        Ok(())
    }
}
