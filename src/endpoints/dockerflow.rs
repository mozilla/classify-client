use crate::{endpoints::EndpointState, errors::ClassifyError};
use actix_web::{web::Data, HttpResponse};
use serde_derive::Serialize;
use std::{
    fs::File,
    io::Read,
    net::{IpAddr, Ipv4Addr},
};

pub async fn lbheartbeat() -> HttpResponse {
    HttpResponse::Ok().body("")
}

#[derive(Serialize)]
struct HeartbeatResponse {
    geoip: bool,
}

pub async fn heartbeat(app_data: Data<EndpointState>) -> Result<HttpResponse, ClassifyError> {
    let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

    app_data
        .geoip
        .locate(ip)
        .and_then(|res| match res {
            Some(country_info) => country_info
                .country
                .and_then(|country| country.iso_code)
                .map(|iso_code| Ok(!iso_code.is_empty()))
                .unwrap_or(Ok(false)),
            None => Ok(false),
        })
        .or(Ok(false))
        .map(|res| {
            let mut resp = if res {
                HttpResponse::Ok()
            } else {
                HttpResponse::ServiceUnavailable()
            };
            resp.json(HeartbeatResponse { geoip: res })
        })
}

pub async fn version(app_data: Data<EndpointState>) -> HttpResponse {
    // Read the file or deliberately fail with a 500 if missing.
    let mut file = File::open(&app_data.version_file).unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();
    HttpResponse::Ok()
        .content_type("application/json")
        .body(data)
}

#[cfg(test)]
mod tests {
    use crate::endpoints::EndpointState;
    use actix_web::{
        http,
        test::{self, TestRequest},
        web::{self, Data},
        App,
    };

    #[actix_rt::test]
    async fn lbheartbeat() {
        let mut service =
            test::init_service(App::new().route("/", web::get().to(super::lbheartbeat))).await;
        let req = TestRequest::default().to_request();
        let res = test::call_service(&mut service, req).await;
        assert_eq!(res.status(), http::StatusCode::OK);
    }

    #[actix_rt::test]
    async fn heartbeat() {
        let mut service = test::init_service(
            App::new()
                .app_data(Data::new(EndpointState::default()))
                .route("/", web::get().to(super::heartbeat)),
        )
        .await;
        let request = TestRequest::default().to_request();
        let response = test::call_service(&mut service, request).await;
        // Should return service unavailable since there is no geoip set up
        assert_eq!(response.status(), http::StatusCode::SERVICE_UNAVAILABLE);
    }

    #[actix_rt::test]
    async fn version() -> Result<(), Box<dyn std::error::Error>> {
        let mut service = test::init_service(
            App::new()
                .app_data(Data::new(EndpointState::default()))
                .route("/", web::get().to(super::version)),
        )
        .await;
        let request = TestRequest::default().to_request();
        let response = test::call_service(&mut service, request).await;
        let status = response.status();
        assert_eq!(status, http::StatusCode::OK);
        Ok(())
    }
}
