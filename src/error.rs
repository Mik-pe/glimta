use thiserror::Error;

use crate::command::CommandError;

/// Errors produced by Glimta.
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[cfg(feature = "network")]
    #[error("mDNS discovery failed: {0}")]
    Discovery(#[from] mdns_sd::Error),
    #[error("mDNS discovery channel closed unexpectedly")]
    DiscoveryChannelClosed,
    #[error("no classic TRADFRI gateway was found before the discovery timeout")]
    NoGatewayFound,
    #[error("gateway returned CoAP status {status} for {path}")]
    GatewayStatus { status: String, path: String },
    #[error("{0} must not be empty")]
    EmptyCredential(&'static str),
    #[error("an observe command must be started with an observation API")]
    ObserveCommandRequiresSubscription,
    #[error(
        "an observation dropped {dropped} stale update(s); the next item is the newest snapshot"
    )]
    ObservationLagged { dropped: u64 },
    #[error("an observation payload could not be decoded: {0}")]
    ObservationDecode(String),
    #[error("an observation transport failed: {message}")]
    ObservationTransport {
        kind: std::io::ErrorKind,
        message: String,
    },
    #[error("an observation ended; a resilient observation will reconnect")]
    ObservationEnded,
    #[error("an observation reconnect attempt failed: {0}")]
    ObservationReconnect(String),
}

/// Glimta result type.
pub type Result<T> = std::result::Result<T, Error>;
