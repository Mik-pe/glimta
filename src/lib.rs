#![forbid(unsafe_code)]
//! Local-first building blocks for the classic IKEA TRÅDFRI gateway.
//!
//! Glimta keeps wire-format knowledge separate from network transport so the
//! protocol can be exercised against captured payloads without a physical hub.

pub mod command;
pub mod model;
pub mod protocol;

pub use command::{Command, CommandError, Method};
pub use model::{Device, DeviceInfo, DeviceKind, Light};
