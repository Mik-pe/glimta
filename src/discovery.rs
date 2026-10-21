use std::{collections::BTreeMap, net::SocketAddr, time::Duration};

use mdns_sd::{ServiceDaemon, ServiceEvent};
use tokio::time::Instant;

use crate::{Error, Gateway, Result, protocol};

pub(crate) async fn discover_gateways(timeout: Duration) -> Result<Vec<Gateway>> {
    let daemon = ServiceDaemon::new()?;
    let receiver = daemon.browse(protocol::DISCOVERY_SERVICE)?;
    let deadline = Instant::now() + timeout;
    let mut discovered = BTreeMap::<String, SocketAddr>::new();

    loop {
        let event = match tokio::time::timeout_at(deadline, receiver.recv_async()).await {
            Ok(Ok(event)) => event,
            Ok(Err(_)) => {
                stop_discovery(&daemon);
                return Err(Error::DiscoveryChannelClosed);
            }
            Err(_) => break,
        };

        let ServiceEvent::ServiceResolved(service) = event else {
            continue;
        };
        let hostname = service.get_hostname();
        if !hostname.starts_with(protocol::GATEWAY_HOST_PREFIX) {
            continue;
        }

        let port = if service.get_port() == 0 {
            protocol::DEFAULT_PORT
        } else {
            service.get_port()
        };
        for address in service.get_addresses_v4() {
            record_gateway(
                &mut discovered,
                hostname,
                SocketAddr::new(address.into(), port),
            );
        }
    }

    stop_discovery(&daemon);
    gateways_from_map(discovered)
}

fn record_gateway(
    discovered: &mut BTreeMap<String, SocketAddr>,
    hostname: &str,
    address: SocketAddr,
) {
    discovered
        .entry(hostname.to_owned())
        .and_modify(|current| {
            if address < *current {
                *current = address;
            }
        })
        .or_insert(address);
}

fn gateways_from_map(discovered: BTreeMap<String, SocketAddr>) -> Result<Vec<Gateway>> {
    if discovered.is_empty() {
        return Err(Error::NoGatewayFound);
    }
    Ok(discovered
        .into_iter()
        .map(|(hostname, address)| Gateway::from_discovery(address, hostname))
        .collect())
}

fn stop_discovery(daemon: &ServiceDaemon) {
    let _ = daemon.stop_browse(protocol::DISCOVERY_SERVICE);
    let _ = daemon.shutdown();
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    #[test]
    fn discovery_order_and_address_selection_are_deterministic() {
        let mut discovered = BTreeMap::new();
        record_gateway(
            &mut discovered,
            "TRADFRI-Gateway-z",
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 20)), 5684),
        );
        record_gateway(
            &mut discovered,
            "TRADFRI-Gateway-a",
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 30)), 5684),
        );
        record_gateway(
            &mut discovered,
            "TRADFRI-Gateway-a",
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)), 5684),
        );

        let gateways = gateways_from_map(discovered).expect("gateways exist");
        assert_eq!(gateways.len(), 2);
        assert_eq!(gateways[0].hostname(), Some("TRADFRI-Gateway-a"));
        assert_eq!(
            gateways[0].address(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)), 5684)
        );
        assert_eq!(gateways[1].hostname(), Some("TRADFRI-Gateway-z"));
    }
}
