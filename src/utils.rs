use actix_web::HttpRequest;
use std::net::IpAddr;

use crate::errors::ClassifyError;

pub trait RequestExt {
    /// Determine the IP address of the client making a request, based on network
    /// information and headers.
    ///
    /// Actix has a method to do this, but it returns a string, and doesn't strip
    /// off ports if present, so it is difficult to use.
    fn client_ip(&self) -> Result<IpAddr, ClassifyError>;
}

impl<S> RequestExt for HttpRequest<S> {
    fn client_ip(&self) -> Result<IpAddr, ClassifyError> {
        if let Some(x_forwarded_for) = self.headers().get("X-Forwarded-For") {
            let ips: Vec<_> = x_forwarded_for
                .to_str()?
                .split(',')
                .map(|ip| ip.trim())
                .collect();
            if ips.len() == 1 {
                return Ok(ips[0].parse()?);
            } else if ips.len() > 1 {
                // the last item is probably a google load balancer, strip that off, use the second-to-last item.
                return Ok(ips[ips.len() - 2].parse()?);
            }
            // 0 items is an empty header, and weird. fall back to peer address detection
        }

        // No headers were present, so use the peer address directly
        if let Some(peer_addr) = self.peer_addr() {
            return Ok(peer_addr.ip());
        }

        Err(ClassifyError::new("Could not determine IP"))
    }
}
