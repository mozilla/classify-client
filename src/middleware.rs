use crate::endpoints::EndpointState;
use actix_web::{
    middleware::{Finished, Middleware, Started},
    HttpRequest, HttpResponse,
};
use cadence::prelude::*;
use std::time::Instant;

pub struct ResponseMetrics;

struct RequestStart(Instant);

impl Middleware<EndpointState> for ResponseMetrics {
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
mod tests {
    use crate::{endpoints::EndpointState, utils::tests::TestMetricSink};
    use actix_web::{
        middleware::{self, Middleware},
        test::TestRequest,
        HttpResponse,
    };
    use cadence::StatsdClient;
    use regex::Regex;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_response_metrics_works() -> Result<(), Box<dyn std::error::Error>> {
        let _sys = actix::System::new("test");
        let log = Arc::new(Mutex::new(Vec::new()));
        let state = EndpointState {
            metrics: StatsdClient::from_sink("test", TestMetricSink { log: log.clone() }),
            ..EndpointState::default()
        };

        let request = TestRequest::with_state(state).finish();
        let middleware = super::ResponseMetrics;
        assert_eq!(
            log.lock().unwrap().len(),
            0,
            "no metrics should be logged yet"
        );

        match middleware.start(&request) {
            Ok(middleware::Started::Done) => (),
            _ => assert!(false, "Middleware should return success synchronously"),
        };
        assert_eq!(
            log.lock().unwrap().len(),
            1,
            "one metric should be logged by start"
        );

        let response = HttpResponse::Ok().finish();

        match middleware.finish(&request, &response) {
            middleware::Finished::Done => (),
            _ => assert!(false, "Middleware should finish synchronously"),
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
        let middleware = super::ResponseMetrics;

        middleware.start(&request).unwrap();
        middleware.finish(&request, &response);

        let log = log.lock().unwrap();
        let response_re = Regex::new(r"test.response:\d+|ms|#status:error")?;
        assert!(response_re.is_match(&log[1]));

        Ok(())
    }
}
