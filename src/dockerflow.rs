use actix_web::{HttpRequest, HttpResponse};
use serde_derive::Serialize;
use std::fs::File;
use std::io::Read;

use crate::api::State;

#[derive(Serialize)]
struct HeartbeatResponse {
    geoip: bool,
}

impl Default for HeartbeatResponse {
    fn default() -> Self {
        Self { geoip: false }
    }
}

pub fn heartbeat(_req: &HttpRequest<State>) -> HttpResponse {
    let mut res = HeartbeatResponse::default();

    // Test GeoIP was loaded.
    res.geoip = true;

    HttpResponse::Ok().json(res)
}

pub fn lbheartbeat(_req: &HttpRequest<State>) -> HttpResponse {
    HttpResponse::Ok().body("")
}

pub fn version(_req: &HttpRequest<State>) -> HttpResponse {
    let mut file = File::open("./version.json").unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();
    HttpResponse::Ok()
        .content_type("application/json")
        .body(data)
}
