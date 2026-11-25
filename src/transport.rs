use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use coap::{
    client::{ClientTransport, CoAPClient},
    request::{Method as CoapMethod, RequestBuilder},
};
use dtls::{cipher_suite::CipherSuiteId, config::Config as DtlsConfig, conn::DTLSConn};
use tokio::net::UdpSocket;

use crate::{Command, Credentials, Error, Method, Result};

pub(crate) type DtlsClient = CoAPClient<DtlsTransport>;

pub(crate) struct DtlsTransport {
    conn: Arc<DTLSConn>,
    remote_addr: SocketAddr,
}

#[async_trait]
impl ClientTransport for DtlsTransport {
    async fn recv(&self, buf: &mut [u8]) -> io::Result<(usize, Option<SocketAddr>)> {
        let read = self
            .conn
            .read(buf, None)
            .await
            .map_err(|error| dtls_io_error(&error))?;
        Ok((read, Some(self.remote_addr)))
    }

    async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.conn
            .write(buf, None)
            .await
            .map_err(|error| dtls_io_error(&error))
    }
}

pub(crate) fn build_dtls_config(credentials: &Credentials) -> DtlsConfig {
    let key = credentials.pre_shared_key().as_bytes().to_vec();
    DtlsConfig {
        psk: Some(Arc::new(move |_| {
            let key = key.clone();
            Box::pin(async move { Ok(key) })
        })),
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
    let bind_address = match address {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let socket = UdpSocket::bind(bind_address).await?;
    socket.connect(address).await?;

    let conn = tokio::time::timeout(
        request_timeout,
        DTLSConn::new(Arc::new(socket), build_dtls_config(credentials), true, None),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "DTLS handshake timed out"))?
    .map_err(|error| dtls_io_error(&error))?;

    let transport = DtlsTransport {
        conn: Arc::new(conn),
        remote_addr: address,
    };
    let mut client = CoAPClient::from_transport(transport);
    client.set_receive_timeout(request_timeout);
    client.set_transport_retries(retries);
    Ok(client)
}

pub(crate) async fn execute_on<T: ClientTransport + 'static>(
    client: &CoAPClient<T>,
    command: &Command,
) -> Result<Vec<u8>> {
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
        .token(Some(rand::random::<[u8; 8]>().to_vec()))
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

fn dtls_io_error(error: &dtls::Error) -> io::Error {
    io::Error::other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn delayed_duplicate_cannot_answer_the_next_read() {
        use coap::request::{MessageClass, MessageType, Packet, Status};
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let client = coap::UdpCoAPClient::new(socket.local_addr().unwrap())
            .await
            .unwrap();
        let gateway = tokio::spawn(async move {
            let mut previous: Option<Box<[u8]>> = None;
            for value in [11, 22] {
                let mut buf = [0; 256];
                let (len, peer) = socket.recv_from(&mut buf).await.unwrap();
                let request = Packet::from_bytes(&buf[..len]).unwrap();
                if let Some(ref duplicate) = previous {
                    socket.send_to(duplicate, peer).await.unwrap();
                }
                let mut response = Packet::new();
                response.header.set_type(MessageType::Acknowledgement);
                response.header.code = MessageClass::Response(Status::Content);
                response.header.message_id = request.header.message_id;
                response.set_token(request.get_token().to_vec());
                response.payload = vec![value];
                let bytes = response.to_bytes().unwrap();
                socket.send_to(&bytes, peer).await.unwrap();
                previous = Some(bytes.into_boxed_slice());
            }
        });
        for id in [11u8, 22] {
            assert_eq!(
                execute_on(&client, &crate::command::get_device(id.into()))
                    .await
                    .unwrap(),
                [id]
            );
        }
        gateway.await.unwrap();
    }

    #[test]
    fn classic_gateway_dtls_config_uses_psk_ccm8() {
        let credentials = Credentials::new("client", "secret").expect("valid credentials");
        let config = build_dtls_config(&credentials);

        assert!(config.psk.is_some());
        assert_eq!(
            config.psk_identity_hint.as_deref(),
            Some(b"client".as_slice())
        );
        assert_eq!(
            config.cipher_suites,
            vec![CipherSuiteId::Tls_Psk_With_Aes_128_Ccm_8]
        );
    }
}
