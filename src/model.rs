use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Device metadata reported in attribute `3` by the gateway.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceInfo {
    #[serde(rename = "0", default)]
    pub manufacturer: Option<String>,
    #[serde(rename = "1", default)]
    pub model_number: Option<String>,
    #[serde(rename = "2", default)]
    pub serial: Option<String>,
    #[serde(rename = "3", default)]
    pub firmware_version: Option<String>,
    #[serde(rename = "6", default)]
    pub power_source: Option<u8>,
    #[serde(rename = "9", default)]
    pub battery_level: Option<u8>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// A single light endpoint nested below device attribute `3311`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Light {
    #[serde(rename = "9003")]
    pub id: u32,
    #[serde(rename = "5850", default)]
    state: u8,
    #[serde(rename = "5851", default)]
    pub brightness: Option<u16>,
    #[serde(rename = "5711", default)]
    pub color_temperature_mireds: Option<u16>,
    #[serde(rename = "5706", default)]
    pub color_hex: Option<String>,
    #[serde(rename = "5707", default)]
    pub hue: Option<u16>,
    #[serde(rename = "5708", default)]
    pub saturation: Option<u16>,
    #[serde(rename = "5709", default)]
    pub color_x: Option<u16>,
    #[serde(rename = "5710", default)]
    pub color_y: Option<u16>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Light {
    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.state == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Light,
    Other,
}

/// A resource returned from `/15001/{id}`.
///
/// Unknown attributes are preserved in `extra`; old gateways and third-party
/// Zigbee devices are allowed to be weird without making deserialization fail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    #[serde(rename = "9003")]
    pub id: u32,
    #[serde(rename = "9001", default)]
    pub display_name: Option<String>,
    #[serde(rename = "5750", default)]
    pub application_type: Option<u32>,
    #[serde(rename = "3", default)]
    pub info: DeviceInfo,
    #[serde(rename = "9020", default)]
    pub last_seen: Option<u64>,
    #[serde(rename = "9019", default)]
    reachable: Option<u8>,
    #[serde(rename = "3311", default)]
    pub lights: Vec<Light>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Device {
    #[must_use]
    pub fn name(&self) -> &str {
        self.display_name.as_deref().unwrap_or("Unnamed device")
    }

    #[must_use]
    pub fn is_reachable(&self) -> Option<bool> {
        self.reachable.map(|value| value == 1)
    }

    #[must_use]
    pub fn kind(&self) -> DeviceKind {
        if self.lights.is_empty() {
            DeviceKind::Other
        } else {
            DeviceKind::Light
        }
    }

    #[must_use]
    pub fn primary_light(&self) -> Option<&Light> {
        self.lights.first()
    }
}

/// Successful response from the gateway identity provisioning endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProvisionedIdentity {
    #[serde(rename = "9091")]
    pub pre_shared_key: String,
}
