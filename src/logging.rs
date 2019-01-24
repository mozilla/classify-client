use std::io;

use actix_web::middleware::{Finished, Middleware, Started};
use actix_web::{HttpRequest, HttpResponse, Result};
use slog::{self, Drain};
use slog_async;
use slog_derive::KV;
use slog_mozlog_json::MozLogJson;
use slog_term;

pub fn get_logger<S: Into<String>>(prefix: S, human_logs: bool) -> slog::Logger {
    let prefix = prefix.into();
    let drain = if human_logs {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        slog_async::Async::new(drain).build().fuse()
    } else {
        let drain = MozLogJson::new(io::stdout())
            .logger_name(format!(
                "{}-{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))
            .msg_type(prefix)
            .build()
            .fuse();
        slog_async::Async::new(drain).build().fuse()
    };

    slog::Logger::root(drain, slog::o!())
}

#[derive(KV)]
struct MozLogFields {
    method: String,
    path: String,
    code: u16,
    agent: Option<String>,
    remote: Option<String>,
    lang: Option<String>,
}

impl MozLogFields {
    fn new<S>(request: &HttpRequest<S>, resp: &HttpResponse) -> Self {
        let headers = request.headers();
        MozLogFields {
            method: request.method().to_string(),
            path: request.uri().to_string(),
            code: resp.status().as_u16(),
            agent: headers
                .get("User-Agent")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.to_string()),
            lang: headers
                .get("Accept-Language")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.to_string()),
            remote: request.connection_info().remote().map(|r| r.to_string()),
        }
    }
}

pub struct RequestLogMiddleware {
    log: slog::Logger,
}

impl RequestLogMiddleware {
    pub fn new(human_logs: bool) -> Self {
        RequestLogMiddleware {
            log: get_logger("request.summary", human_logs),
        }
    }
}

impl<S> Middleware<S> for RequestLogMiddleware {
    fn start(&self, _req: &HttpRequest<S>) -> Result<Started> {
        Ok(Started::Done)
    }

    fn finish(&self, request: &HttpRequest<S>, resp: &HttpResponse) -> Finished {
        let fields = MozLogFields::new(request, resp);
        slog::info!(self.log, "" ; slog::o!(fields));
        Finished::Done
    }
}

impl Default for RequestLogMiddleware {
    fn default() -> Self {
        Self {
            log: slog::Logger::root(slog::Discard, slog::o!()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::logging::MozLogFields;
    use actix_web::{http, test, HttpResponse};

    #[test]
    fn test_request_fields() {
        let request = test::TestRequest::with_header("User-Agent", "test-request").finish();
        let response = HttpResponse::build(http::StatusCode::CREATED).finish();

        let fields = MozLogFields::new(&request, &response);

        assert_eq!(fields.method, "GET");
        assert_eq!(fields.path, "/");
        assert_eq!(fields.code, 201);
        assert_eq!(fields.agent, Some("test-request".into()));
        assert_eq!(fields.lang, None);
        assert_eq!(fields.remote, None);
    }
}
