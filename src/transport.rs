use std::{net::SocketAddr, sync::Arc, time::Duration};

use coap::{
    client::CoAPClient,
    dtls::{DtlsConnection, UdpDtlsConfig},
    request::{Method as CoapMethod, RequestBuilder},
};
use webrtc_dtls::{cipher_suite::CipherSuiteId, config::Config as DtlsConfig};

use crate::{Command, Credentials, Error, Method, Result};

pub(crate) type DtlsClient = CoAPClient<DtlsConnection>;

pub(crate) fn build_dtls_config(credentials: &Credentials) -> DtlsConfig {
    let key = credentials.pre_shared_key().as_bytes().to_vec();
    DtlsConfig {
        psk: Some(Arc::new(move |_| Ok(key.clone()))),
        // webrtc-dtls uses this field as the client PSK identity when dialing.
        psk_identity_hint: Some(credentials.identity().as_bytes().to_vec()),
        cipher_suites: vec![CipherSuiteId::Tls_Psk_With_Aes_128_Ccm_8],
        ..Default::default()
    }
}

pub(crate) async fn open_client(
    address: SocketAddr,
    credentials: &Credentials,
    request_timeout: Duration,
    retries: usize,
) -> Result<DtlsClient> {
    let mut client = CoAPClient::from_udp_dtls_config(UdpDtlsConfig {
        config: build_dtls_config(credentials),
        dest_addr: address,
    })
    .await?;
    client.set_receive_timeout(request_timeout);
    client.set_transport_retries(retries);
    Ok(client)
}

pub(crate) async fn execute_on(client: &DtlsClient, command: &Command) -> Result<Vec<u8>> {
    if command.observe {
        return Err(Error::ObserveCommandRequiresSubscription);
    }

    let method = match command.method {
        Method::Get => CoapMethod::Get,
        Method::Put => CoapMethod::Put,
        Method::Post => CoapMethod::Post,
    };
    let body = command.body.as_ref().map(serde_json::to_vec).transpose()?;
    let request = RequestBuilder::new(&command.path, method)
        .data(body)
        .confirmable(true)
        .build();
    let response = client.send(request).await?;
    let status = response.get_status();
    if status.is_error() {
        return Err(Error::GatewayStatus {
            status: format!("{status:?}"),
            path: command.path.clone(),
        });
    }
    Ok(response.message.payload)
}
