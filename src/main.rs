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
    settings::Settings,
};

const APP_NAME: &str = "classify-client";

fn main() -> Result<(), ClassifyError> {
    let settings = Settings::load()?;

    let _guard = sentry::init(settings.sentry_dsn.clone());
    sentry::integrations::panic::register_panic_handler();

    let sys = actix::System::new(APP_NAME);

    let app_log = logging::get_logger("app", &settings);

    let metrics = metrics::get_client(&settings, app_log.clone()).unwrap_or_else(|err| {
        panic!(format!(
            "Critical failure setting up metrics logging: {}",
            err
        ))
    });

    let state = EndpointState {
        geoip: geoip::get_arbiter(&settings, metrics.clone()),
        metrics,
        settings: settings.clone(),
        log: app_log.clone(),
    };

    let addr = format!("{}:{}", state.settings.host, state.settings.port);
    let server = actix_web::server::new(move || {
        let mut app = App::with_state(state.clone())
            .middleware(SentryMiddleware::new())
            .middleware(metrics::ResponseMiddleware)
            .middleware(logging::RequestLogMiddleware::new(&settings))
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
    slog::info!(app_log, "started server on https://{}", addr);
    sys.run();

    Ok(())
}
