#![allow(clippy::missing_errors_doc)]

use std::{
    cmp,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use coap::client::ObserveMessage;
use futures_core::Stream;
use rand::random;
use serde::de::DeserializeOwned;
use tokio::{
    sync::{broadcast, oneshot},
    task::JoinHandle,
};
use tokio_stream::{
    StreamExt,
    wrappers::{BroadcastStream, errors::BroadcastStreamRecvError},
};

use crate::{
    AirPurifier, Blind, Command, Credentials, Device, Error, Result, Socket, command, discovery,
    model::{Group, ProvisionedIdentity},
    protocol, transport,
};

const OBSERVATION_BUFFER_CAPACITY: usize = 1;
const MIN_RECONNECT_DELAY: Duration = Duration::from_millis(100);

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

/// Backoff used by opt-in resilient observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectOptions {
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl Default for ReconnectOptions {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        }
    }
}

impl ReconnectOptions {
    fn normalized(self) -> Self {
        let initial_delay = cmp::max(self.initial_delay, MIN_RECONNECT_DELAY);
        Self {
            initial_delay,
            max_delay: cmp::max(self.max_delay, initial_delay),
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

    /// Discover classic gateways announced through mDNS in deterministic order.
    pub async fn discover_all(timeout: Duration) -> Result<Vec<Self>> {
        discovery::discover_gateways(timeout).await
    }

    /// Discover one classic gateway.
    ///
    /// When several gateways are present, the first item from [`Self::discover_all`]
    /// is returned. Use `discover_all` when the caller needs to choose explicitly.
    pub async fn discover(timeout: Duration) -> Result<Self> {
        let mut gateways = Self::discover_all(timeout).await?;
        Ok(gateways.remove(0))
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

/// A failed member of a best-effort bulk read.
#[derive(Debug)]
pub struct ResourceFailure {
    pub id: u32,
    pub error: Error,
}

/// Results from a best-effort bulk read.
#[derive(Debug)]
pub struct BulkRead<T> {
    pub items: Vec<T>,
    pub failures: Vec<ResourceFailure>,
}

impl<T> BulkRead<T> {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            failures: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.failures.is_empty()
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
    ///
    /// This strict form preserves the original behavior and returns on the first
    /// failed device. Use [`Self::devices_best_effort`] when stale or malformed
    /// devices must not hide otherwise healthy resources.
    pub async fn devices(&self) -> Result<Vec<Device>> {
        let client = self.open().await?;
        let ids: Vec<u32> = decode_on(&client, &command::list_devices()).await?;
        let mut devices = Vec::with_capacity(ids.len());
        for id in ids {
            devices.push(decode_on(&client, &command::get_device(id)).await?);
        }
        Ok(devices)
    }

    /// Fetch every readable device and report individual failures separately.
    pub async fn devices_best_effort(&self) -> Result<BulkRead<Device>> {
        let client = self.open().await?;
        let ids: Vec<u32> = decode_on(&client, &command::list_devices()).await?;
        let mut read = BulkRead::with_capacity(ids.len());
        for id in ids {
            record_resource_result(
                &mut read,
                id,
                decode_on(&client, &command::get_device(id)).await,
            );
        }
        Ok(read)
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
    ///
    /// This strict form preserves the original behavior and returns on the first
    /// failed group. Use [`Self::groups_best_effort`] for partial snapshots.
    pub async fn groups(&self) -> Result<Vec<Group>> {
        let client = self.open().await?;
        let ids: Vec<u32> = decode_on(&client, &command::list_groups()).await?;
        let mut groups = Vec::with_capacity(ids.len());
        for id in ids {
            groups.push(decode_on(&client, &command::get_group(id)).await?);
        }
        Ok(groups)
    }

    /// Fetch every readable group and report individual failures separately.
    pub async fn groups_best_effort(&self) -> Result<BulkRead<Group>> {
        let client = self.open().await?;
        let ids: Vec<u32> = decode_on(&client, &command::list_groups()).await?;
        let mut read = BulkRead::with_capacity(ids.len());
        for id in ids {
            record_resource_result(
                &mut read,
                id,
                decode_on(&client, &command::get_group(id)).await,
            );
        }
        Ok(read)
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

    /// Observe a device and automatically create a new DTLS/CoAP observation
    /// after a transport failure or unexpected end.
    ///
    /// Transient failures remain visible as stream errors. The stream stays
    /// alive and retries with bounded exponential backoff until cancelled.
    pub async fn observe_device_resilient(
        &self,
        device_id: u32,
        reconnect: ReconnectOptions,
    ) -> Result<Observation<Device>> {
        self.observe_json_resilient(protocol::device_path(device_id), reconnect)
            .await
    }

    /// Resilient counterpart to [`Self::observe_group`].
    pub async fn observe_group_resilient(
        &self,
        group_id: u32,
        reconnect: ReconnectOptions,
    ) -> Result<Observation<Group>> {
        self.observe_json_resilient(protocol::group_path(group_id), reconnect)
            .await
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
        T: DeserializeOwned + Clone + Send + 'static,
    {
        let client = self.open().await?;
        let (sender, receiver) = observation_channel();
        let callback_sender = sender.clone();
        let cancel = client
            .observe(&path, move |message| {
                let parsed = match message {
                    Ok(message) => serde_json::from_slice(&message.payload)
                        .map_err(|error| ObservationFailure::Decode(error.to_string())),
                    Err(error) => Err(ObservationFailure::Transport {
                        kind: error.kind(),
                        message: error.to_string(),
                    }),
                };
                let _ = callback_sender.send(parsed);
            })
            .await?;
        drop(sender);

        Ok(Observation {
            receiver,
            control: ObservationControl::Direct {
                cancel: Some(cancel),
                _client: client,
            },
        })
    }

    async fn observe_json_resilient<T>(
        &self,
        path: String,
        reconnect: ReconnectOptions,
    ) -> Result<Observation<T>>
    where
        T: DeserializeOwned + Clone + Send + 'static,
    {
        let initial = self.observe_json(path.clone()).await?;
        let reconnect = reconnect.normalized();
        let (sender, receiver) = observation_channel();
        let (cancel, mut cancelled) = oneshot::channel();
        let client = self.clone();

        let task = tokio::spawn(async move {
            supervise_observation(
                client,
                path,
                initial,
                reconnect,
                sender,
                &mut cancelled,
            )
            .await;
        });

        Ok(Observation {
            receiver,
            control: ObservationControl::Resilient {
                cancel: Some(cancel),
                task,
            },
        })
    }
}

fn record_resource_result<T>(read: &mut BulkRead<T>, id: u32, result: Result<T>) {
    match result {
        Ok(item) => read.items.push(item),
        Err(error) => read.failures.push(ResourceFailure { id, error }),
    }
}

async fn decode_on<T>(client: &transport::DtlsClient, command: &Command) -> Result<T>
where
    T: DeserializeOwned,
{
    let payload = transport::execute_on(client, command).await?;
    Ok(serde_json::from_slice(&payload)?)
}

#[derive(Debug, Clone)]
enum ObservationFailure {
    Transport {
        kind: std::io::ErrorKind,
        message: String,
    },
    Decode(String),
    Lagged(u64),
    Ended,
    Reconnect(String),
}

impl ObservationFailure {
    fn into_error(self) -> Error {
        match self {
            Self::Transport { kind, message } => Error::ObservationTransport { kind, message },
            Self::Decode(message) => Error::ObservationDecode(message),
            Self::Lagged(dropped) => Error::ObservationLagged { dropped },
            Self::Ended => Error::ObservationEnded,
            Self::Reconnect(message) => Error::ObservationReconnect(message),
        }
    }
}

type ObservationMessage<T> = std::result::Result<T, ObservationFailure>;

fn observation_channel<T>() -> (
    broadcast::Sender<ObservationMessage<T>>,
    BroadcastStream<ObservationMessage<T>>,
)
where
    T: Clone + Send + 'static,
{
    let (sender, receiver) = broadcast::channel(OBSERVATION_BUFFER_CAPACITY);
    (sender, BroadcastStream::new(receiver))
}

enum ObservationControl {
    Direct {
        cancel: Option<oneshot::Sender<ObserveMessage>>,
        _client: transport::DtlsClient,
    },
    Resilient {
        cancel: Option<oneshot::Sender<()>>,
        task: JoinHandle<()>,
    },
}

/// A cancellable CoAP Observe subscription.
///
/// The queue is intentionally bounded to the newest snapshot. If a consumer
/// falls behind, stale snapshots are discarded and an
/// [`Error::ObservationLagged`] item is emitted before the newest state.
///
/// Direct observations end after transport failure. Use the resilient observe
/// methods when automatic resubscription is desired.
#[must_use = "dropping the observation immediately cancels it"]
pub struct Observation<T>
where
    T: Clone + Send + 'static,
{
    receiver: BroadcastStream<ObservationMessage<T>>,
    control: ObservationControl,
}

impl<T> Observation<T>
where
    T: Clone + Send + 'static,
{
    pub async fn recv(&mut self) -> Option<Result<T>> {
        map_observation_item(self.receiver.next().await)
    }

    pub fn cancel(mut self) {
        self.terminate();
    }

    fn terminate(&mut self) {
        match &mut self.control {
            ObservationControl::Direct { cancel, .. } => {
                if let Some(cancel) = cancel.take() {
                    let _ = cancel.send(ObserveMessage::Terminate);
                }
            }
            ObservationControl::Resilient { cancel, task } => {
                if let Some(cancel) = cancel.take() {
                    let _ = cancel.send(());
                }
                task.abort();
            }
        }
    }
}

impl<T> Unpin for Observation<T> where T: Clone + Send + 'static {}

impl<T> Stream for Observation<T>
where
    T: Clone + Send + 'static,
{
    type Item = Result<T>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.receiver).poll_next(context) {
            Poll::Ready(item) => Poll::Ready(map_observation_item(item)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T> Drop for Observation<T>
where
    T: Clone + Send + 'static,
{
    fn drop(&mut self) {
        self.terminate();
    }
}

fn map_observation_item<T>(
    item: Option<std::result::Result<ObservationMessage<T>, BroadcastStreamRecvError>>,
) -> Option<Result<T>> {
    match item {
        Some(Ok(Ok(value))) => Some(Ok(value)),
        Some(Ok(Err(error))) => Some(Err(error.into_error())),
        Some(Err(BroadcastStreamRecvError::Lagged(dropped))) => {
            Some(Err(Error::ObservationLagged { dropped }))
        }
        None => None,
    }
}

async fn supervise_observation<T>(
    client: Client,
    path: String,
    initial: Observation<T>,
    reconnect: ReconnectOptions,
    sender: broadcast::Sender<ObservationMessage<T>>,
    cancelled: &mut oneshot::Receiver<()>,
) where
    T: DeserializeOwned + Clone + Send + 'static,
{
    let mut current = Some(initial);
    let mut delay = reconnect.initial_delay;

    loop {
        let mut observation = if let Some(initial) = current.take() {
            initial
        } else {
            let sleep = tokio::time::sleep(delay);
            tokio::pin!(sleep);
            tokio::select! {
                _ = &mut *cancelled => return,
                () = &mut sleep => {}
            }

            match client.observe_json(path.clone()).await {
                Ok(observation) => observation,
                Err(error) => {
                    if sender
                        .send(Err(ObservationFailure::Reconnect(error.to_string())))
                        .is_err()
                    {
                        return;
                    }
                    delay = next_reconnect_delay(delay, reconnect.max_delay);
                    continue;
                }
            }
        };

        let mut saw_update = false;
        loop {
            tokio::select! {
                _ = &mut *cancelled => return,
                item = observation.recv() => {
                    match item {
                        Some(Ok(value)) => {
                            saw_update = true;
                            delay = reconnect.initial_delay;
                            if sender.send(Ok(value)).is_err() {
                                return;
                            }
                        }
                        Some(Err(Error::ObservationLagged { dropped })) => {
                            if sender.send(Err(ObservationFailure::Lagged(dropped))).is_err() {
                                return;
                            }
                        }
                        Some(Err(Error::ObservationDecode(message))) => {
                            if sender.send(Err(ObservationFailure::Decode(message))).is_err() {
                                return;
                            }
                        }
                        Some(Err(Error::ObservationTransport { kind, message })) => {
                            let _ = sender.send(Err(ObservationFailure::Transport { kind, message }));
                            break;
                        }
                        Some(Err(error)) => {
                            let _ = sender.send(Err(ObservationFailure::Reconnect(error.to_string())));
                            break;
                        }
                        None => {
                            let _ = sender.send(Err(ObservationFailure::Ended));
                            break;
                        }
                    }
                }
            }
        }

        drop(observation);
        if !saw_update {
            delay = next_reconnect_delay(delay, reconnect.max_delay);
        }
    }
}

fn next_reconnect_delay(current: Duration, maximum: Duration) -> Duration {
    cmp::min(current.saturating_mul(2), maximum)
}

// Keep these imported endpoint types visible in generated docs alongside Device.
const _: Option<(AirPurifier, Blind, Socket)> = None;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_effort_read_keeps_successes_and_failures() {
        let mut read = BulkRead::with_capacity(2);
        record_resource_result(&mut read, 1, Ok("healthy"));
        record_resource_result::<&str>(&mut read, 2, Err(Error::NoGatewayFound));

        assert_eq!(read.items, vec!["healthy"]);
        assert_eq!(read.failures.len(), 1);
        assert_eq!(read.failures[0].id, 2);
        assert!(!read.is_complete());
    }

    #[tokio::test]
    async fn observation_buffer_reports_lag_then_keeps_latest_snapshot() {
        let (sender, mut receiver) = observation_channel::<u32>();
        sender.send(Ok(1)).expect("receiver exists");
        sender.send(Ok(2)).expect("receiver exists");

        assert!(matches!(
            receiver.next().await,
            Some(Err(BroadcastStreamRecvError::Lagged(1)))
        ));
        assert!(matches!(receiver.next().await, Some(Ok(Ok(2)))));
    }

    #[test]
    fn reconnect_backoff_is_bounded_and_never_hot_loops() {
        let normalized = ReconnectOptions {
            initial_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
        }
        .normalized();

        assert_eq!(normalized.initial_delay, MIN_RECONNECT_DELAY);
        assert_eq!(normalized.max_delay, MIN_RECONNECT_DELAY);
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(20), Duration::from_secs(30)),
            Duration::from_secs(30)
        );
    }
}
