use actix_web::HttpRequest;
use std::net::IpAddr;

use crate::{errors::ClassifyError, endpoints::EndpointState};

pub trait RequestClientIp<S> {
    /// Determine the IP address of the client making a request, based on network
    /// information and headers.
    ///
    /// Actix has a method to do this, but it returns a string, and doesn't strip
    /// off ports if present, so it is difficult to use.
    fn client_ip(&self) -> Result<IpAddr, ClassifyError>;
}

pub trait RequestPathIps<'a, S> {
    /// Iterate all known proxy and client IPs, starting with the IPs closest to
    /// the server, and ending with the alleged client.
    fn iter_path_ips(&'a self) -> PathIpsIter<'a, S>;
}

impl RequestClientIp<EndpointState> for HttpRequest<EndpointState> {
    fn client_ip(&self) -> Result<IpAddr, ClassifyError> {
        let trusted_proxy_ip_ranges: Vec<ipnet::IpNet> =
            self.state().settings.trusted_proxy_ip_ranges()?;

        let is_trusted_ip = |ip: &IpAddr| {
            trusted_proxy_ip_ranges
                .iter()
                .any(|range| range.contains(ip))
        };

        self.iter_path_ips()
            .skip_while(is_trusted_ip)
            .next()
            .ok_or_else(|| ClassifyError::new("Could not determine IP"))
    }
}

impl<'a, S> RequestPathIps<'a, S> for HttpRequest<S> {
    fn iter_path_ips(&'a self) -> PathIpsIter<'a, S> {
        PathIpsIter::new(self)
    }
}

pub struct PathIpsIter<'a, S> {
    request: &'a HttpRequest<S>,
    state: PathIpsIterState,
}

enum PathIpsIterState {
    Peer,
    XForwardedFor(usize),
    Done,
}

impl<'a, S> PathIpsIter<'a, S> {
    fn new(request: &'a HttpRequest<S>) -> Self {
        Self {
            request,
            state: PathIpsIterState::Peer,
        }
    }
}

impl<'a, S> Iterator for PathIpsIter<'a, S> {
    type Item = IpAddr;

    fn next(&mut self) -> Option<Self::Item> {
        // A state machine that processes first a possible peer addr, and then
        // each of the IPs in the X-Forwarded-For header. It is important to
        // note that the state machine can make several state transitions per
        // function call. This is because at each state, it could find that
        // there isn't anything to yield, and needs to advance to another state
        // to get a value (or know there are no more items).

        loop {
            match self.state {
                PathIpsIterState::Peer => {
                    self.state = PathIpsIterState::XForwardedFor(0);
                    // Get the network IP, if available
                    if let Some(peer_addr) = self.request.peer_addr() {
                        return Some(peer_addr.ip());
                    }
                }

                PathIpsIterState::XForwardedFor(idx_from_end) => {
                    // Get a list of IPs from the header, and the return the next in the sequence
                    // TODO it would be nice to not have get the list of parsed IPs from the header every time.
                    if let Some(x_forwarded_for) = self.request.headers().get("X-Forwarded-For") {
                        if let Ok(header) = x_forwarded_for.to_str() {
                            let ips: Vec<_> = header.split(',').map(|ip| ip.trim()).collect();
                            if idx_from_end < ips.len() {
                                self.state = PathIpsIterState::XForwardedFor(idx_from_end + 1);
                                if let Ok(ip) = ips[ips.len() - idx_from_end - 1].parse() {
                                    return Some(ip);
                                }
                            } else {
                                self.state = PathIpsIterState::Done;
                            }
                        } else {
                            self.state = PathIpsIterState::Done;
                        }
                    } else {
                        self.state = PathIpsIterState::Done;
                    }
                }

                PathIpsIterState::Done => {
                    return None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn path_ip_iter_works() {
        let req =
            TestRequest::with_header("x-forwarded-for", "1.2.3.4, 5.6.7.8, 9.10.11.12").finish();
        let path_ips: Vec<_> = req.iter_path_ips().collect();
        let expected = vec![
            IpAddr::V4(Ipv4Addr::new(9, 10, 11, 12)),
            IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8)),
            IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
        ];
        assert_eq!(
            path_ips, expected,
            "IPs in x-forwarded-for should be iterated in reverse order"
        );
    }
}
