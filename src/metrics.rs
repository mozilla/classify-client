use crate::{endpoints::EndpointState, errors::ClassifyError, APP_NAME};
use actix_web::{
    middleware::{Finished, Middleware, Started},
    HttpRequest, HttpResponse,
};
use cadence::{prelude::*, BufferedUdpMetricSink, StatsdClient};
use std::{
    fmt::Display,
    net::{ToSocketAddrs, UdpSocket},
    time::Instant,
};

pub fn get_client<A>(metrics_target: A, log: slog::Logger) -> Result<StatsdClient, ClassifyError>
where
    A: ToSocketAddrs + Display,
{
    let builder = {
        // Bind a socket to any/all interfaces (0.0.0.0) and an arbitrary
        // port, chosen by the OS (indicated by port 0). This port is used
        // only to send metrics data, and isn't used to receive anything.

        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        match BufferedUdpMetricSink::from(&metrics_target, socket) {
            Ok(udp_sink) => {
                let sink = cadence::QueuingMetricSink::from(udp_sink);
                StatsdClient::builder(APP_NAME, sink)
            }
            Err(err) => {
                slog::error!(
                    log,
                    "Could not connect to metrics host on {}: {}",
                    metrics_target,
                    err,
                );
                let sink = cadence::NopMetricSink;
                StatsdClient::builder(APP_NAME, sink)
            }
        }
    };
    Ok(builder
        .with_error_handler(move |error| slog::error!(log, "Could not send metric: {}", error))
        .build())
}

pub struct ResponseMiddleware;

struct RequestStart(Instant);

impl Middleware<EndpointState> for ResponseMiddleware {
    fn start(&self, req: &HttpRequest<EndpointState>) -> actix_web::Result<Started> {
        req.extensions_mut().insert(RequestStart(Instant::now()));
        req.state()
            .metrics
            .incr_with_tags("ongoing_requests")
            .send();
        Ok(Started::Done)
    }

    fn finish(&self, req: &HttpRequest<EndpointState>, resp: &HttpResponse) -> Finished {
        if let Some(RequestStart(started)) = req.extensions().get::<RequestStart>() {
            let duration = started.elapsed();
            req.state()
                .metrics
                .time_duration_with_tags("response", duration)
                .with_tag(
                    "status",
                    if resp.status().is_success() {
                        "success"
                    } else {
                        "error"
                    },
                )
                .send();
        }
        req.state()
            .metrics
            .decr_with_tags("ongoing_requests")
            .send();
        Finished::Done
    }
}

#[cfg(test)]
pub mod tests {
    use crate::endpoints::EndpointState;
    use actix_web::{
        middleware::{self, Middleware},
        test::TestRequest,
        HttpResponse,
    };
    use cadence::StatsdClient;
    use regex::Regex;
    use std::{
        io,
        sync::{Arc, Mutex},
    };

    #[derive(Clone, Debug)]
    pub struct TestMetricSink {
        pub log: Arc<Mutex<Vec<String>>>,
    }

    impl cadence::MetricSink for TestMetricSink {
        fn emit(&self, metric: &str) -> io::Result<usize> {
            let mut log = self.log.lock().unwrap();
            log.push(metric.to_owned());
            Ok(0)
        }
    }

    #[test]
    fn test_response_metrics_works() -> Result<(), Box<dyn std::error::Error>> {
        let _sys = actix::System::new("test");
        let log = Arc::new(Mutex::new(Vec::new()));
        let state = EndpointState {
            metrics: StatsdClient::from_sink("test", TestMetricSink { log: log.clone() }),
            ..EndpointState::default()
        };

        let request = TestRequest::with_state(state).finish();
        let middleware = super::ResponseMiddleware;
        assert_eq!(
            log.lock().unwrap().len(),
            0,
            "no metrics should be logged yet"
        );

        match middleware.start(&request) {
            Ok(middleware::Started::Done) => (),
            _ => panic!("Middleware should return success synchronously"),
        };
        assert_eq!(
            log.lock().unwrap().len(),
            1,
            "one metric should be logged by start"
        );

        let response = HttpResponse::Ok().finish();

        match middleware.finish(&request, &response) {
            middleware::Finished::Done => (),
            _ => panic!("Middleware should finish synchronously"),
        };
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 3, "one metric should be logged by start");

        assert_eq!(log[0], "test.ongoing_requests:1|c");
        assert_eq!(log[2], "test.ongoing_requests:-1|c");

        let response_re = Regex::new(r"test.response:\d+|ms|#status:success")?;
        assert!(response_re.is_match(&log[1]));

        Ok(())
    }

    #[test]
    fn test_response_metrics_logs_error() -> Result<(), Box<dyn std::error::Error>> {
        let _sys = actix::System::new("test");
        let log = Arc::new(Mutex::new(Vec::new()));
        let state = EndpointState {
            metrics: StatsdClient::from_sink("test", TestMetricSink { log: log.clone() }),
            ..EndpointState::default()
        };

        let request = TestRequest::with_state(state).finish();
        let response = HttpResponse::InternalServerError().finish();
        let middleware = super::ResponseMiddleware;

        middleware.start(&request).unwrap();
        middleware.finish(&request, &response);

        let log = log.lock().unwrap();
        let response_re = Regex::new(r"test.response:\d+|ms|#status:error")?;
        assert!(response_re.is_match(&log[1]));

        Ok(())
    }
}
