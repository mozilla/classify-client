#![deny(clippy::all)]

//! A server that tells clients what time it is and where they are in the world.

#![deny(missing_docs)]

use actix_web::App;
use listenfd::ListenFd;
use maxminddb;
use std::{env, process};

use crate::api::{index, GeoIpActor, State};
use crate::dockerflow::{heartbeat, lbheartbeat, version};

mod api;
mod dockerflow;
mod errors;

fn main() {
    // Rust doesn't have a ctrl-c handler itself, so when running as
    // PID 1 in Docker it doesn't respond to SIGINT. This prevents
    // ctrl-c from stopping a docker container running this
    // program. Handle SIGINT (aka ctrl-c) to fix this problem.
    ctrlc::set_handler(move || process::exit(0)).expect("error setting ctrl-c handler");

    let sys = actix::System::new("classify-client");

    let geoip = actix::SyncArbiter::start(1, || {
        let geoip_path = "./GeoLite2-Country.mmdb";
        GeoIpActor::from_path(&geoip_path).unwrap_or_else(|err| {
            panic!(format!(
                "Could not open geoip database at {:?}: {}",
                geoip_path, err
            ))
        })
    });

    let server = actix_web::server::new(move || {
        App::with_state(State {
            geoip: geoip.clone(),
        })
        .resource("/", |r| r.get().f(index))
        // Dockerflow views
        .resource("/__lbheartbeat__", |r| r.get().f(lbheartbeat))
        .resource("/__heartbeat__", |r| r.get().f(heartbeat))
        .resource("/__version__", |r| r.get().f(version))
    });

    // Re-use a passed file descriptor, or create a new one to listen on.
    let mut listenfd = ListenFd::from_env();
    let server = if let Some(listener) = listenfd
        .take_tcp_listener(0)
        .expect("Could not get TCP listener")
    {
        println!("started server on re-used file descriptor");
        server.listen(listener)
    } else {
        let host = env::var("HOST").unwrap_or_else(|_| "localhost".to_string());
        let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        let addr = format!("{}:{}", host, port);
        println!("started server on https://{}:{}", host, port);
        server
            .bind(&addr)
            .unwrap_or_else(|err| panic!(format!("Couldn't listen on {}: {}", &addr, err)))
    };

    server.start();
    sys.run();
}
