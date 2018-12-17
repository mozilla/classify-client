use actix_web::{FutureResponse, HttpRequest, HttpResponse};
use futures::Future;
use serde_derive::Serialize;
use std::{
    fs::File,
    io::Read,
    net::{IpAddr, Ipv4Addr},
};

use crate::{geoip::CountryForIp, views::ViewState};

pub fn lbheartbeat<S>(_req: &HttpRequest<S>) -> HttpResponse {
    HttpResponse::Ok().body("")
}

#[derive(Serialize)]
struct HeartbeatResponse {
    geoip: bool,
}

pub fn heartbeat(req: &HttpRequest<ViewState>) -> FutureResponse<HttpResponse> {
    let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

    Box::new(
        req.state()
            .geoip
            .send(CountryForIp::new(ip))
            .and_then(|res| match res {
                Ok(country_info) => country_info
                    .and_then(|country_info| country_info.country)
                    .and_then(|country| country.iso_code)
                    .and_then(|iso_code| Some(Ok(iso_code == "US")))
                    .unwrap_or(Ok(false)),
                Err(_) => Ok(false),
            })
            .or_else(|_| Ok(false))
            .and_then(|res| {
                let mut resp = if res {
                    HttpResponse::Ok()
                } else {
                    HttpResponse::ServiceUnavailable()
                };
                Ok(resp.json(HeartbeatResponse { geoip: res }))
            }),
    )
}

pub fn version(req: &HttpRequest<ViewState>) -> HttpResponse {
    let version_file = &req.state().settings.version_file;
    // Read the file or deliberately fail with a 500 if missing.
    let mut file = File::open(version_file).unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();
    HttpResponse::Ok()
        .content_type("application/json")
        .body(data)
}
