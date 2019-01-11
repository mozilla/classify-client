//! A server that tells clients what time it is and where they are in the world.
//!
#![deny(clippy::all)]

mod endpoints;
mod errors;
mod geoip;
mod logging;
mod middleware;
mod settings;
mod utils;

use actix_web::App;
use cadence::{BufferedUdpMetricSink, StatsdClient};
use sentry;
use sentry_actix::SentryMiddleware;
use slog;
use std::net::UdpSocket;

use crate::{
    endpoints::{classify, debug, dockerflow, EndpointState},
    errors::ClassifyError,
    geoip::GeoIpActor,
    middleware::ResponseMetrics,
    settings::Settings,
};

fn main() -> Result<(), ClassifyError> {
    let settings = Settings::load()?;

    let _guard = sentry::init(settings.sentry_dsn.clone());
    sentry::integrations::panic::register_panic_handler();

    let sys = actix::System::new("classify-client");

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

    let metrics: StatsdClient = {
        let log_metrics = app_log.clone();
        let builder = {
            let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
            socket.set_nonblocking(true).unwrap();
            match BufferedUdpMetricSink::from(&settings.metrics_target, socket) {
                Ok(udp_sink) => {
                    let sink = cadence::QueuingMetricSink::from(udp_sink);
                    StatsdClient::builder("classify-client", sink)
                }
                Err(err) => {
                    slog::error!(
                        log_main.log,
                        "Could not connect to metrics host on {}: {}",
                        &settings.metrics_target,
                        err,
                    );
                    let sink = cadence::NopMetricSink;
                    StatsdClient::builder("classify-client", sink)
                }
            }
        };
        builder
            .with_error_handler(move |error| {
                slog::error!(log_metrics.log, "Could not send metric: {}", error)
            })
            .build()
    };

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
            .middleware(ResponseMetrics)
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
