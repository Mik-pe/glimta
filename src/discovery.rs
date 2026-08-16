use std::{net::SocketAddr, time::Duration};

use mdns_sd::{ServiceDaemon, ServiceEvent};

use crate::{protocol, Gateway, Error, Result};

pub(crate) async fn discover_gateway(timeout: Duration) -> Result<Gateway> {
    let daemon = ServiceDaemon::new()?;
    let receiver = daemon.browse(protocol::DISCOVERY_SERVICE)?;

    let discovered = tokio::time::timeout(timeout, async {
        loop {
            let event = receiver
                .recv_async()
                .await
                .map_err(|_| Error::DiscoveryChannelClosed)?;
            let ServiceEvent::ServiceResolved(service) = event else {
                continue;
            };
            if !service
                .get_hostname()
                .starts_with(protocol::GATEWAY_HOST_PREFIX)
            {
                continue;
            }
            let Some(address) = service.get_addresses_v4().into_iter().next() else {
                continue;
            };
            let port = if service.get_port() == 0 {
                protocol::DEFAULT_PORT
            } else {
                service.get_port()
            };
            return Ok(Gateway::from_discovery(
                SocketAddr::new(address.into(), port),
                service.get_hostname().to_owned(),
            ));
        }
    })
    .await;

    let _ = daemon.stop_browse(protocol::DISCOVERY_SERVICE);
    let _ = daemon.shutdown();

    match discovered {
        Ok(result) => result,
        Err(_) => Err(Error::NoGatewayFound),
    }
}
