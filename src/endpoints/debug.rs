use crate::{endpoints::EndpointState, utils::RequestClientIp};
use actix_web::{HttpRequest, HttpResponse};

/// Show debugging information about the server comprising:
///
///  * Request state,
///  * Current request's headers
///  * Calculated client IP for the current request
///
/// This handler should be disabled in production servers.
pub async fn debug_handler(req: HttpRequest) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "received headers: {:?}\n\nrequest state: {:?}\n\nclient ip: {:?}",
        req.headers(),
        &req.app_data::<EndpointState>(),
        req.client_ip()
    ))
}
