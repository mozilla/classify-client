//! A server that tells clients what time it is and where they are in the world.
//!
#![deny(clippy::all)]

pub mod endpoints;
pub mod errors;
pub mod geoip;
pub mod keys;
pub mod logging;
pub mod metrics;
pub mod settings;
pub mod utils;

use crate::{
    endpoints::{canned, classify, country, debug, dockerflow, EndpointState},
    errors::ClassifyError,
    geoip::GeoIp,
    settings::Settings,
};
use actix_web::{
    web::{self, Data},
    App,
};
use std::sync::Arc;

const APP_NAME: &str = "classify-client";

#[actix_web::main]
async fn main() -> Result<(), ClassifyError> {
    let Settings {
        api_keys_file,
        debug,
        geoip_db_path,
        host,
        human_logs,
        metrics_target,
        log_level,
        port,
        sentry_dsn,
        sentry_env,
        sentry_sample_rate,
        trusted_proxy_list,
        version_file,
        ..
    } = Settings::load()?;

    let app_log = logging::get_logger("app", human_logs, log_level);

    let metrics = Arc::new(
        metrics::get_client(metrics_target, app_log.clone())
            .unwrap_or_else(|err| panic!("Critical failure setting up metrics logging: {err}")),
    );

    let _guard = sentry::init((
        sentry_dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(sentry_env.into()),
            sample_rate: sentry_sample_rate,
            ..Default::default()
        },
    ));

    let state = EndpointState {
        api_keys_hashset: keys::load(api_keys_file, app_log.clone()),
        geoip: Arc::new(
            GeoIp::builder()
                .path(geoip_db_path)
                .metrics(Arc::clone(&metrics))
                .build()?,
        ),
        metrics,
        trusted_proxies: trusted_proxy_list,
        log: app_log.clone(),
        version_file,
    };

    let addr = format!("{host}:{port}");
    slog::info!(app_log, "starting server on https://{}", addr);

    actix_web::HttpServer::new(move || {
        let mut app = App::new()
            .app_data(Data::new(state.clone()))
            .wrap(metrics::ResponseTimer)
            .wrap(logging::RequestLogger)
            .wrap(sentry_actix::Sentry::new())
            // API Endpoints
            .service(web::resource("/").route(web::get().to(classify::classify_client)))
            .service(
                web::resource("/api/v1/classify_client/")
                    .route(web::get().to(classify::classify_client)),
            )
            .service(web::resource("/v1/country").route(web::route().to(country::get_country)))
            // Dockerflow Endpoints
            .service(
                web::resource("/__lbheartbeat__").route(web::get().to(dockerflow::lbheartbeat)),
            )
            .service(web::resource("/__heartbeat__").route(web::get().to(dockerflow::heartbeat)))
            .service(web::resource("/__version__").route(web::get().to(dockerflow::version)))
            // Static responses
            // no /v1/geolocate, intentionally returning 404
            .service(web::resource("/v1/geosubmit").route(web::to(canned::forbidden)))
            .service(web::resource("/v1/submit").route(web::to(canned::forbidden)))
            .service(web::resource("/v2/geosubmit").route(web::to(canned::forbidden)));

        if debug {
            app = app.service(web::resource("/debug").route(web::get().to(debug::debug_handler)));
        }

        app
    })
    .bind(&addr)?
    .run()
    .await?;

    Ok(())
}
