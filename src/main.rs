//! A server that tells clients what time it is and where they are in the world.
//!
#![deny(clippy::all)]
#![deny(missing_docs)]

mod endpoints;
mod errors;
mod geoip;
mod settings;
mod utils;

use actix_web::App;
use sentry;
use sentry_actix::SentryMiddleware;

use crate::{
    endpoints::{classify, dockerflow, EndpointState},
    errors::ClassifyError,
    geoip::GeoIpActor,
    settings::Settings,
};

fn main() -> Result<(), ClassifyError> {
    let sys = actix::System::new("classify-client");

    let settings = Settings::load()?;

    let _guard = sentry::init(settings.sentry_dsn.clone());
    sentry::integrations::panic::register_panic_handler();

    let geoip = {
        let path = settings.geoip_db_path.clone();
        actix::SyncArbiter::start(1, move || {
            GeoIpActor::from_path(&path).unwrap_or_else(|err| {
                panic!(format!(
                    "Could not open geoip database at {:?}: {}",
                    path, err
                ))
            })
        })
    };

    let state = EndpointState { geoip, settings };

    let addr = format!("{}:{}", state.settings.host, state.settings.port);
    let server = actix_web::server::new(move || {
        App::with_state(state.clone())
            .middleware(SentryMiddleware::new())
            .resource("/", |r| r.get().f(classify::classify_client))
            // Dockerflow Endpoints
            .resource("/__lbheartbeat__", |r| r.get().f(dockerflow::lbheartbeat))
            .resource("/__heartbeat__", |r| r.get().f(dockerflow::heartbeat))
            .resource("/__version__", |r| r.get().f(dockerflow::version))
    })
    .bind(&addr)?;

    server.start();
    println!("started server on https://{}", addr);
    sys.run();

    Ok(())
}
