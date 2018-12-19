use actix_web::{HttpRequest, HttpResponse};

use crate::{endpoints::EndpointState, utils::RequestClientIp};

/// Show debugging information about the server comprising:
///
///  * Server settings
///  * Current request's headers
///  * Calculated client IP for the current request
///
/// This handler should be disabled in production servers.
pub fn debug_handler(req: &HttpRequest<EndpointState>) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "received headers: {:?}\n\nsettings: {:?}\n\nclient ip: {:?}",
        req.headers(),
        &req.state().settings,
        req.client_ip()
    ))
}
