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
}

/// Glimta result type.
pub type Result<T> = std::result::Result<T, Error>;
