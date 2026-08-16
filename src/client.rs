#![allow(clippy::missing_errors_doc)]

use std::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use coap::client::ObserveMessage;
use futures_core::Stream;
use rand::random;
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc::UnboundedReceiver, oneshot};

use crate::{
    AirPurifier, Blind, Command, Credentials, Device, Error, Result, Socket, command, discovery,
    model::{Group, ProvisionedIdentity},
    protocol, transport,
};

/// Network behavior for a connected client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientOptions {
    pub request_timeout: Duration,
    pub retries: usize,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(3),
            retries: 3,
        }
    }
}

/// Addressable classic TRADFRI gateway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gateway {
    address: SocketAddr,
    hostname: Option<String>,
}

impl Gateway {
    #[must_use]
    pub const fn new(address: IpAddr) -> Self {
        Self {
            address: SocketAddr::new(address, protocol::DEFAULT_PORT),
            hostname: None,
        }
    }

    #[must_use]
    pub const fn from_socket_addr(address: SocketAddr) -> Self {
        Self {
            address,
            hostname: None,
        }
    }

    pub(crate) fn from_discovery(address: SocketAddr, hostname: String) -> Self {
        Self {
            address,
            hostname: Some(hostname),
        }
    }

    /// Discover the first classic gateway announced through mDNS.
    pub async fn discover(timeout: Duration) -> Result<Self> {
        discovery::discover_gateway(timeout).await
    }

    #[must_use]
    pub const fn address(&self) -> SocketAddr {
        self.address
    }

    #[must_use]
    pub fn hostname(&self) -> Option<&str> {
        self.hostname.as_deref()
    }

    /// Provision a new random client identity with the printed gateway security code.
    pub async fn provision(&self, security_code: &str) -> Result<Credentials> {
        let identity = format!("glimta-{:016x}{:016x}", random::<u64>(), random::<u64>());
        self.provision_with_identity(security_code, &identity).await
    }

    /// Provision a caller-selected long-lived identity.
    pub async fn provision_with_identity(
        &self,
        security_code: &str,
        identity: &str,
    ) -> Result<Credentials> {
        let bootstrap = Credentials::new("Client_identity", security_code)?;
        let client = transport::open_client(
            self.address,
            &bootstrap,
            ClientOptions::default().request_timeout,
            ClientOptions::default().retries,
        )
        .await?;
        let payload =
            transport::execute_on(&client, &command::provision_identity(identity)).await?;
        let provisioned: ProvisionedIdentity = serde_json::from_slice(&payload)?;
        Credentials::new(identity, provisioned.pre_shared_key)
    }

    #[must_use]
    pub fn connect(&self, credentials: Credentials) -> Client {
        Client {
            gateway: self.clone(),
            credentials,
            options: ClientOptions::default(),
        }
    }

    #[must_use]
    pub fn connect_with_options(&self, credentials: Credentials, options: ClientOptions) -> Client {
        Client {
            gateway: self.clone(),
            credentials,
            options,
        }
    }
}

/// Authenticated client for a classic gateway.
#[derive(Debug, Clone)]
pub struct Client {
    gateway: Gateway,
    credentials: Credentials,
    options: ClientOptions,
}

impl Client {
    #[must_use]
    pub fn gateway(&self) -> &Gateway {
        &self.gateway
    }

    #[must_use]
    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    #[must_use]
    pub const fn options(&self) -> ClientOptions {
        self.options
    }

    /// Execute an arbitrary non-observe Glimta command.
    ///
    /// Each call opens a fresh DTLS session. High-level bulk reads deliberately
    /// reuse one read session, while writes remain isolated for compatibility
    /// with gateways that reject repeated PUT operations on a reused session.
    pub async fn execute(&self, command: Command) -> Result<Vec<u8>> {
        let client = self.open().await?;
        transport::execute_on(&client, &command).await
    }

    pub async fn device_ids(&self) -> Result<Vec<u32>> {
        let client = self.open().await?;
        decode_on(&client, &command::list_devices()).await
    }

    pub async fn device(&self, device_id: u32) -> Result<Device> {
        let client = self.open().await?;
        decode_on(&client, &command::get_device(device_id)).await
    }

    /// Fetch all devices while reusing one read-only DTLS session.
    pub async fn devices(&self) -> Result<Vec<Device>> {
        let client = self.open().await?;
        let ids: Vec<u32> = decode_on(&client, &command::list_devices()).await?;
        let mut devices = Vec::with_capacity(ids.len());
        for id in ids {
            devices.push(decode_on(&client, &command::get_device(id)).await?);
        }
        Ok(devices)
    }

    pub async fn group_ids(&self) -> Result<Vec<u32>> {
        let client = self.open().await?;
        decode_on(&client, &command::list_groups()).await
    }

    pub async fn group(&self, group_id: u32) -> Result<Group> {
        let client = self.open().await?;
        decode_on(&client, &command::get_group(group_id)).await
    }

    /// Fetch all groups while reusing one read-only DTLS session.
    pub async fn groups(&self) -> Result<Vec<Group>> {
        let client = self.open().await?;
        let ids: Vec<u32> = decode_on(&client, &command::list_groups()).await?;
        let mut groups = Vec::with_capacity(ids.len());
        for id in ids {
            groups.push(decode_on(&client, &command::get_group(id)).await?);
        }
        Ok(groups)
    }

    pub async fn set_light_state(&self, device_id: u32, on: bool) -> Result<()> {
        self.execute_unit(command::set_light_state(device_id, on))
            .await
    }

    pub async fn set_light_brightness(
        &self,
        device_id: u32,
        brightness: u16,
        transition_time: Option<u16>,
    ) -> Result<()> {
        self.execute_unit(command::set_light_brightness(
            device_id,
            brightness,
            transition_time,
        )?)
        .await
    }

    pub async fn set_light_color_temperature(
        &self,
        device_id: u32,
        mireds: u16,
        transition_time: Option<u16>,
    ) -> Result<()> {
        self.execute_unit(command::set_light_color_temperature(
            device_id,
            mireds,
            transition_time,
        )?)
        .await
    }

    pub async fn set_light_hex_color(
        &self,
        device_id: u32,
        color: &str,
        transition_time: Option<u16>,
    ) -> Result<()> {
        self.execute_unit(command::set_light_hex_color(
            device_id,
            color,
            transition_time,
        )?)
        .await
    }

    pub async fn set_light_xy_color(
        &self,
        device_id: u32,
        x: u16,
        y: u16,
        transition_time: Option<u16>,
    ) -> Result<()> {
        self.execute_unit(command::set_light_xy_color(
            device_id,
            x,
            y,
            transition_time,
        )?)
        .await
    }

    pub async fn set_light_hsb(
        &self,
        device_id: u32,
        hue: u16,
        saturation: u16,
        brightness: Option<u16>,
        transition_time: Option<u16>,
    ) -> Result<()> {
        self.execute_unit(command::set_light_hsb(
            device_id,
            hue,
            saturation,
            brightness,
            transition_time,
        )?)
        .await
    }

    pub async fn set_socket_state(&self, device_id: u32, on: bool) -> Result<()> {
        self.execute_unit(command::set_socket_state(device_id, on))
            .await
    }

    pub async fn set_blind_position(&self, device_id: u32, position: u8) -> Result<()> {
        self.execute_unit(command::set_blind_position(device_id, position)?)
            .await
    }

    pub async fn trigger_blind(&self, device_id: u32) -> Result<()> {
        self.execute_unit(command::trigger_blind(device_id)).await
    }

    pub async fn turn_air_purifier_off(&self, device_id: u32) -> Result<()> {
        self.execute_unit(command::turn_air_purifier_off(device_id))
            .await
    }

    pub async fn set_air_purifier_auto(&self, device_id: u32) -> Result<()> {
        self.execute_unit(command::set_air_purifier_auto(device_id))
            .await
    }

    pub async fn set_air_purifier_fan_speed(&self, device_id: u32, speed: u8) -> Result<()> {
        self.execute_unit(command::set_air_purifier_fan_speed(device_id, speed)?)
            .await
    }

    pub async fn set_air_purifier_controls_locked(
        &self,
        device_id: u32,
        locked: bool,
    ) -> Result<()> {
        self.execute_unit(command::set_air_purifier_controls_locked(device_id, locked))
            .await
    }

    pub async fn set_air_purifier_leds_off(&self, device_id: u32, leds_off: bool) -> Result<()> {
        self.execute_unit(command::set_air_purifier_leds_off(device_id, leds_off))
            .await
    }

    pub async fn set_group_state(&self, group_id: u32, on: bool) -> Result<()> {
        self.execute_unit(command::set_group_state(group_id, on))
            .await
    }

    pub async fn set_group_brightness(
        &self,
        group_id: u32,
        brightness: u16,
        transition_time: Option<u16>,
    ) -> Result<()> {
        self.execute_unit(command::set_group_brightness(
            group_id,
            brightness,
            transition_time,
        )?)
        .await
    }

    pub async fn set_group_color_temperature(
        &self,
        group_id: u32,
        mireds: u16,
        transition_time: Option<u16>,
    ) -> Result<()> {
        self.execute_unit(command::set_group_color_temperature(
            group_id,
            mireds,
            transition_time,
        )?)
        .await
    }

    pub async fn observe_device(&self, device_id: u32) -> Result<Observation<Device>> {
        self.observe_json(protocol::device_path(device_id)).await
    }

    pub async fn observe_group(&self, group_id: u32) -> Result<Observation<Group>> {
        self.observe_json(protocol::group_path(group_id)).await
    }

    async fn execute_unit(&self, command: Command) -> Result<()> {
        self.execute(command).await.map(|_| ())
    }

    async fn open(&self) -> Result<transport::DtlsClient> {
        transport::open_client(
            self.gateway.address,
            &self.credentials,
            self.options.request_timeout,
            self.options.retries,
        )
        .await
    }

    async fn observe_json<T>(&self, path: String) -> Result<Observation<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let client = self.open().await?;
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let cancel = client
            .observe(&path, move |message| {
                let parsed = message.map_err(Error::Io).and_then(|message| {
                    serde_json::from_slice(&message.payload).map_err(Error::from)
                });
                let _ = sender.send(parsed);
            })
            .await?;
        Ok(Observation {
            receiver,
            cancel: Some(cancel),
            _client: client,
        })
    }
}

async fn decode_on<T>(client: &transport::DtlsClient, command: &Command) -> Result<T>
where
    T: DeserializeOwned,
{
    let payload = transport::execute_on(client, command).await?;
    Ok(serde_json::from_slice(&payload)?)
}

/// A cancellable CoAP Observe subscription.
///
/// It can be consumed with [`Observation::recv`] or as a `Stream`. Dropping it
/// explicitly terminates the observe relationship instead of leaving the
/// underlying CoAP observation alive.
#[must_use = "dropping the observation immediately cancels it"]
pub struct Observation<T> {
    receiver: UnboundedReceiver<Result<T>>,
    cancel: Option<oneshot::Sender<ObserveMessage>>,
    _client: transport::DtlsClient,
}

impl<T> Observation<T> {
    pub async fn recv(&mut self) -> Option<Result<T>> {
        self.receiver.recv().await
    }

    pub fn cancel(mut self) {
        self.terminate();
    }

    fn terminate(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(ObserveMessage::Terminate);
        }
    }
}

impl<T> Unpin for Observation<T> {}

impl<T> Stream for Observation<T> {
    type Item = Result<T>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().receiver).poll_recv(context)
    }
}

impl<T> Drop for Observation<T> {
    fn drop(&mut self) {
        self.terminate();
    }
}

// Keep these imported endpoint types visible in generated docs alongside Device.
const _: Option<(AirPurifier, Blind, Socket)> = None;
