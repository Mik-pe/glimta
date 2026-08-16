#![forbid(unsafe_code)]
//! Local-first Rust support for the classic IKEA TRADFRI gateway.
//!
//! Glimta separates the numeric gateway wire format, typed resources, command
//! construction, and network transport. The optional network layer performs
//! mDNS discovery and CoAP over DTLS-PSK without requiring a cloud service.

pub mod command;
pub mod credentials;
pub mod error;
pub mod model;
pub mod protocol;

#[cfg(feature = "network")]
mod discovery;
#[cfg(feature = "network")]
mod transport;
#[cfg(feature = "network")]
pub mod client;

pub use command::{Command, CommandError, Method};
pub use credentials::Credentials;
pub use error::{Error, Result};
pub use model::{
    AirPurifier, Blind, Device, DeviceInfo, DeviceKind, Group, Light, SignalRepeater, Socket,
};

#[cfg(feature = "network")]
pub use client::{Client, ClientOptions, Gateway, Observation};
