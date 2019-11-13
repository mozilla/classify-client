use crate::{endpoints::EndpointState, errors::ClassifyError};
use actix_web::{web::Data, HttpResponse};
use serde_derive::Serialize;
use std::{
    fs::File,
    io::Read,
    net::{IpAddr, Ipv4Addr},
};

pub fn lbheartbeat() -> HttpResponse {
    HttpResponse::Ok().body("")
}

#[derive(Serialize)]
struct HeartbeatResponse {
    geoip: bool,
}

pub fn heartbeat(app_data: Data<EndpointState>) -> Result<HttpResponse, ClassifyError> {
    let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

    app_data
        .geoip
        .locate(ip)
        .and_then(|res| match res {
            Some(country_info) => country_info
                .country
                .and_then(|country| country.iso_code)
                .and_then(|iso_code| Some(Ok(!iso_code.is_empty())))
                .unwrap_or(Ok(false)),
            None => Ok(false),
        })
        .or_else(|_| Ok(false))
        .and_then(|res| {
            let mut resp = if res {
                HttpResponse::Ok()
            } else {
                HttpResponse::ServiceUnavailable()
            };
            Ok(resp.json(HeartbeatResponse { geoip: res }))
        })
}

pub fn version(app_data: Data<EndpointState>) -> HttpResponse {
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
        web, App,
    };

    #[test]
    fn lbheartbeat() {
        let mut service =
            test::init_service(App::new().route("/", web::get().to(super::lbheartbeat)));
        let req = TestRequest::default().to_request();
        let res = test::call_service(&mut service, req);
        assert_eq!(res.status(), http::StatusCode::OK);
    }

    #[test]
    fn heartbeat() {
        let mut service = test::init_service(
            App::new()
                .data(EndpointState::default())
                .route("/", web::get().to(super::heartbeat)),
        );
        let request = TestRequest::default().to_request();
        let response = test::call_service(&mut service, request);
        // Should return service unavailable since there is no geoip set up
        assert_eq!(response.status(), http::StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn version() -> Result<(), Box<dyn std::error::Error>> {
        let mut service = test::init_service(
            App::new()
                .data(EndpointState::default())
                .route("/", web::get().to(super::version)),
        );
        let request = TestRequest::default().to_request();
        let response = test::call_service(&mut service, request);
        let status = response.status();
        assert_eq!(status, http::StatusCode::OK);
        Ok(())
    }
}
