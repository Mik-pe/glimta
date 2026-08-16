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

/// A switched outlet endpoint nested below device attribute `3312`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Socket {
    #[serde(rename = "9003")]
    pub id: u32,
    #[serde(rename = "5850", default)]
    state: u8,
    #[serde(rename = "5852", default)]
    pub on_time: Option<u64>,
    #[serde(rename = "5805", default)]
    pub cumulative_active_power: Option<f64>,
    #[serde(rename = "5820", default)]
    pub power_factor: Option<f64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Socket {
    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.state == 1
    }
}

/// A blind endpoint nested below device attribute `15015`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blind {
    #[serde(rename = "9003")]
    pub id: u32,
    #[serde(rename = "5536")]
    pub current_position: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// An air purifier endpoint nested below device attribute `15025`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AirPurifier {
    #[serde(rename = "9003")]
    pub id: u32,
    #[serde(rename = "5900", default)]
    pub mode: Option<i64>,
    #[serde(rename = "5902", default)]
    pub filter_runtime: Option<i64>,
    #[serde(rename = "5903", default)]
    filter_status: Option<i64>,
    #[serde(rename = "5904", default)]
    pub filter_lifetime_total: Option<i64>,
    #[serde(rename = "5905", default)]
    controls_locked: Option<i64>,
    #[serde(rename = "5906", default)]
    leds_off: Option<i64>,
    #[serde(rename = "5907", default)]
    pub air_quality: Option<i64>,
    #[serde(rename = "5908", default)]
    pub fan_speed: Option<i64>,
    #[serde(rename = "5909", default)]
    pub motor_runtime_total: Option<i64>,
    #[serde(rename = "5910", default)]
    pub filter_lifetime_remaining: Option<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl AirPurifier {
    #[must_use]
    pub fn is_on(&self) -> Option<bool> {
        self.mode.map(|mode| mode > 0)
    }

    #[must_use]
    pub fn is_auto_mode(&self) -> Option<bool> {
        self.mode.map(|mode| mode == 1)
    }

    #[must_use]
    pub fn controls_locked(&self) -> Option<bool> {
        self.controls_locked.map(|value| value == 1)
    }

    #[must_use]
    pub fn leds_off(&self) -> Option<bool> {
        self.leds_off.map(|value| value == 1)
    }

    #[must_use]
    pub fn filter_needs_replacement(&self) -> Option<bool> {
        self.filter_status.map(|value| value != 0)
    }
}

/// A signal repeater control endpoint.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SignalRepeater {
    #[serde(rename = "9003", default)]
    pub id: Option<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceKind {
    Light,
    Socket,
    Blind,
    AirPurifier,
    SignalRepeater,
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
    #[serde(rename = "3312", default)]
    pub sockets: Vec<Socket>,
    #[serde(rename = "15015", default)]
    pub blinds: Vec<Blind>,
    #[serde(rename = "15025", default)]
    pub air_purifiers: Vec<AirPurifier>,
    #[serde(rename = "15014", default)]
    pub signal_repeaters: Vec<SignalRepeater>,
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
    pub fn capabilities(&self) -> Vec<DeviceKind> {
        let mut kinds = Vec::with_capacity(5);
        if !self.lights.is_empty() {
            kinds.push(DeviceKind::Light);
        }
        if !self.sockets.is_empty() {
            kinds.push(DeviceKind::Socket);
        }
        if !self.blinds.is_empty() {
            kinds.push(DeviceKind::Blind);
        }
        if !self.air_purifiers.is_empty() {
            kinds.push(DeviceKind::AirPurifier);
        }
        if !self.signal_repeaters.is_empty() {
            kinds.push(DeviceKind::SignalRepeater);
        }
        if kinds.is_empty() {
            kinds.push(DeviceKind::Other);
        }
        kinds
    }

    #[must_use]
    pub fn kind(&self) -> DeviceKind {
        self.capabilities()[0]
    }

    #[must_use]
    pub fn primary_light(&self) -> Option<&Light> {
        self.lights.first()
    }
}

/// A group resource returned from `/15004/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    #[serde(rename = "9003")]
    pub id: u32,
    #[serde(rename = "9001", default)]
    pub display_name: Option<String>,
    #[serde(rename = "5850", default)]
    state: u8,
    #[serde(rename = "5851", default)]
    pub brightness: Option<u16>,
    #[serde(rename = "5706", default)]
    pub color_hex: Option<String>,
    #[serde(rename = "9039", default)]
    pub mood_id: Option<u32>,
    #[serde(rename = "9018", default)]
    pub raw_members: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Group {
    #[must_use]
    pub fn name(&self) -> &str {
        self.display_name.as_deref().unwrap_or("Unnamed group")
    }

    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.state == 1
    }

    #[must_use]
    pub fn member_ids(&self) -> Vec<u32> {
        self.raw_members
            .get("15002")
            .and_then(|links| links.get("9003"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_u64)
            .filter_map(|id| u32::try_from(id).ok())
            .collect()
    }
}

/// Successful response from the gateway identity provisioning endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProvisionedIdentity {
    #[serde(rename = "9091")]
    pub pre_shared_key: String,
}
