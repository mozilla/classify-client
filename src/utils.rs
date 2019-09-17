use crate::{endpoints::EndpointState, errors::ClassifyError};
use actix_web::HttpRequest;
use std::net::IpAddr;

pub trait RequestClientIp<S> {
    /// Determine the IP address of the client making a request, based on network
    /// information and headers.
    ///
    /// Actix has a method to do this, but it returns a string, and doesn't strip
    /// off ports if present, so it is difficult to use.
    fn client_ip(&self) -> Result<IpAddr, ClassifyError>;
}

pub trait RequestTraceIps<'a> {
    /// Iterate all known proxy and client IPs, starting with the IPs closest to
    /// the server, and ending with the alleged client.
    fn trace_ips(&'a self) -> Vec<IpAddr>;
}

impl RequestClientIp<EndpointState> for HttpRequest {
    fn client_ip(&self) -> Result<IpAddr, ClassifyError> {
        let trusted_proxy_list = &self
            .app_data::<EndpointState>()
            .expect("Expected app state")
            .trusted_proxies;

        let is_trusted_ip =
            |ip: &&IpAddr| trusted_proxy_list.iter().any(|range| range.contains(*ip));

        self.trace_ips()
            .iter()
            .skip_while(is_trusted_ip)
            .next()
            .ok_or_else(|| ClassifyError::new("Could not determine IP"))
            .map(|ip| *ip)
    }
}

impl<'a> RequestTraceIps<'a> for HttpRequest {
    fn trace_ips(&'a self) -> Vec<IpAddr> {
        let mut trace: Vec<IpAddr> = Vec::new();

        if let Some(peer_addr) = self.peer_addr() {
            trace.push(peer_addr.ip());
        }

        if let Some(x_forwarded_for) = self.headers().get("X-Forwarded-For") {
            if let Ok(header) = x_forwarded_for.to_str() {
                let mut header_ips: Vec<IpAddr> =
                    header.split(',').flat_map(|ip| ip.trim().parse()).collect();
                header_ips.reverse();
                trace.append(&mut header_ips);
            }
        }

        trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn trace_ip_works() {
        let req = TestRequest::with_header("x-forwarded-for", "1.2.3.4, 5.6.7.8, 9.10.11.12")
            .to_http_request();
        assert_eq!(
            req.trace_ips(),
            vec![
                IpAddr::V4(Ipv4Addr::new(9, 10, 11, 12)),
                IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8)),
                IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
            ],
            "IPs in x-forwarded-for should be iterated in reverse order",
        );
    }

    // Note that in all of the below tests, there aren't any networks involved,
    // so the requests don't have a peer address. As such, the X-Forwarded-For
    // header is the only thing considered to determine the client IP. Actix
    // doesn't seem to provide a way to create a request with a mocked peer
    // address.

    #[test]
    fn get_client_ip_no_proxies() -> Result<(), Box<dyn std::error::Error + 'static>> {
        let _sys = actix::System::new("test");
        let state = EndpointState::default();
        assert_eq!(
            state.trusted_proxies.len(),
            0,
            "Precondition: no trusted proxies by default"
        );

        let req = TestRequest::with_header("x-forwarded-for", "1.2.3.4, 5.6.7.8")
            .data(state)
            .to_http_request();

        assert_eq!(
            req.client_ip()?,
            IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8)),
            "With no proxies, the right-most ip should be used"
        );

        Ok(())
    }

    #[test]
    fn get_client_ip_one_proxies() -> Result<(), Box<dyn std::error::Error + 'static>> {
        let _sys = actix::System::new("test");
        let state = EndpointState {
            trusted_proxies: vec!["5.6.7.8/32".parse()?],
            ..EndpointState::default()
        };

        let req = TestRequest::with_header("x-forwarded-for", "1.2.3.4, 5.6.7.8")
            .data(state)
            .to_http_request();

        assert_eq!(
            req.client_ip()?,
            IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
            "With one proxy, the second-from-the-right ip should be used"
        );

        Ok(())
    }

    #[test]
    fn get_client_ip_too_many_proxies() -> Result<(), Box<dyn std::error::Error + 'static>> {
        let _sys = actix::System::new("test");
        let state = EndpointState {
            trusted_proxies: vec!["5.6.7.8/32".parse()?, "1.2.3.4/32".parse()?],
            ..EndpointState::default()
        };

        let req = TestRequest::with_header("x-forwarded-for", "1.2.3.4, 5.6.7.8")
            .data(state)
            .to_http_request();

        assert!(
            req.client_ip().is_err(),
            "With too many proxies configured, no ip is given"
        );

        Ok(())
    }
}
