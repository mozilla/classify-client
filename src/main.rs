//! A server that tells clients what time it is and where they are in the world.
//!
#![deny(clippy::all)]

pub mod endpoints;
pub mod errors;
pub mod geoip;
pub mod logging;
pub mod metrics;
pub mod settings;
pub mod utils;

use actix_web::App;
use sentry;
use sentry_actix::SentryMiddleware;
use slog;

use crate::{
    endpoints::{classify, debug, dockerflow, EndpointState},
    errors::ClassifyError,
    geoip::GeoIpActor,
    settings::Settings,
};

const APP_NAME: &str = "classify-client";

fn main() -> Result<(), ClassifyError> {
    let settings = Settings::load()?;

    let _guard = sentry::init(settings.sentry_dsn.clone());
    sentry::integrations::panic::register_panic_handler();

    let sys = actix::System::new(APP_NAME);

    let app_log = if settings.human_logs {
        logging::MozLogger::new_human()
    } else {
        logging::MozLogger::new_json("app")
    };
    let log_main = app_log.clone();

    let request_log = if settings.human_logs {
        logging::MozLogger::new_human()
    } else {
        logging::MozLogger::new_json("request.summary")
    };

    let metrics = metrics::get_client(&settings, app_log.clone().log).unwrap_or_else(|err| {
        panic!(format!(
            "Critical failure setting up metrics logging: {}",
            err
        ))
    });

    let geoip = {
        let path = settings.geoip_db_path.clone();
        let geoip_metrics = metrics.clone();
        actix::SyncArbiter::start(1, move || {
            GeoIpActor::builder()
                .path(&path)
                .metrics(geoip_metrics.clone())
                .build()
                .unwrap_or_else(|err| {
                    panic!(format!(
                        "Could not open geoip database at {:?}: {}",
                        path, err
                    ))
                })
        })
    };

    let state = EndpointState {
        geoip,
        metrics,
        settings: settings.clone(),
        log: app_log.clone(),
    };

    let addr = format!("{}:{}", state.settings.host, state.settings.port);
    let server = actix_web::server::new(move || {
        let mut app = App::with_state(state.clone())
            .middleware(SentryMiddleware::new())
            .middleware(metrics::ResponseMiddleware)
            .middleware(request_log.clone())
            // API Endpoints
            .resource("/", |r| r.get().f(classify::classify_client))
            .resource("/api/v1/classify_client/", |r| {
                r.get().f(classify::classify_client)
            })
            // Dockerflow Endpoints
            .resource("/__lbheartbeat__", |r| r.get().f(dockerflow::lbheartbeat))
            .resource("/__heartbeat__", |r| r.get().f(dockerflow::heartbeat))
            .resource("/__version__", |r| r.get().f(dockerflow::version));

        if settings.debug {
            app = app.resource("/debug", |r| r.get().f(debug::debug_handler));
        }

        app
    })
    .bind(&addr)?;

    server.start();
    slog::info!(log_main.log, "started server on https://{}", addr);
    sys.run();

    Ok(())
}
