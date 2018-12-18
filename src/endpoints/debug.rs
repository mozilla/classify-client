use actix_web::{HttpRequest, HttpResponse};

use crate::{endpoints::EndpointState, utils::RequestClientIp};

/// Show settings
pub fn debug_handler(req: &HttpRequest<EndpointState>) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "received headers: {:?}\n\nsettings: {:?}\n\nclient ip: {:?}",
        req.headers(),
        &req.state().settings,
        req.client_ip()
    ))
}
