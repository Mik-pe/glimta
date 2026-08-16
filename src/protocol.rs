//! TRÅDFRI CoAP paths and numeric attribute identifiers.
//!
//! IKEA's classic gateway represents resources as JSON objects whose keys are
//! LwM2M/IPSO-style numeric strings. Keeping them in one module makes the wire
//! format explicit without leaking magic numbers through the rest of the crate.

pub const DEFAULT_PORT: u16 = 5684;
pub const DISCOVERY_SERVICE: &str = "_coap._udp.local.";
pub const GATEWAY_HOST_PREFIX: &str = "TRADFRI-Gateway-";

pub const ROOT_DEVICES: &str = "15001";
pub const ROOT_GROUPS: &str = "15004";
pub const ROOT_MOODS: &str = "15005";
pub const ROOT_SMART_TASKS: &str = "15010";
pub const ROOT_GATEWAY: &str = "15011";
pub const ROOT_SIGNAL_REPEATER: &str = "15014";
pub const ROOT_BLINDS: &str = "15015";
pub const ROOT_AIR_PURIFIER: &str = "15025";

pub const ATTR_DEVICE_INFO: &str = "3";
pub const ATTR_NAME: &str = "9001";
pub const ATTR_CREATED_AT: &str = "9002";
pub const ATTR_ID: &str = "9003";
pub const ATTR_REACHABLE: &str = "9019";
pub const ATTR_LAST_SEEN: &str = "9020";
pub const ATTR_GROUP_MEMBERS: &str = "9018";
pub const ATTR_GROUP_ID: &str = "9038";
pub const ATTR_MOOD: &str = "9039";
pub const ATTR_HS_LINK: &str = "15002";
pub const ATTR_AUTH: &str = "9063";
pub const ATTR_CLIENT_IDENTITY: &str = "9090";
pub const ATTR_PSK: &str = "9091";
pub const ATTR_APPLICATION_TYPE: &str = "5750";

pub const ATTR_DEVICE_MANUFACTURER: &str = "0";
pub const ATTR_DEVICE_MODEL_NUMBER: &str = "1";
pub const ATTR_DEVICE_SERIAL: &str = "2";
pub const ATTR_DEVICE_FIRMWARE_VERSION: &str = "3";
pub const ATTR_DEVICE_POWER_SOURCE: &str = "6";
pub const ATTR_DEVICE_BATTERY: &str = "9";

pub const ATTR_LIGHT_CONTROL: &str = "3311";
pub const ATTR_SOCKET_CONTROL: &str = "3312";
pub const ATTR_DEVICE_STATE: &str = "5850";
pub const ATTR_LIGHT_DIMMER: &str = "5851";
pub const ATTR_SOCKET_ON_TIME: &str = "5852";
pub const ATTR_SOCKET_CUM_ACTIVE_POWER: &str = "5805";
pub const ATTR_SOCKET_POWER_FACTOR: &str = "5820";
pub const ATTR_LIGHT_COLOR_HEX: &str = "5706";
pub const ATTR_LIGHT_COLOR_HUE: &str = "5707";
pub const ATTR_LIGHT_COLOR_SATURATION: &str = "5708";
pub const ATTR_LIGHT_COLOR_X: &str = "5709";
pub const ATTR_LIGHT_COLOR_Y: &str = "5710";
pub const ATTR_LIGHT_MIREDS: &str = "5711";
pub const ATTR_TRANSITION_TIME: &str = "5712";

pub const ATTR_BLIND_CURRENT_POSITION: &str = "5536";
pub const ATTR_BLIND_TRIGGER: &str = "5523";

pub const ATTR_AIR_PURIFIER_MODE: &str = "5900";
pub const ATTR_AIR_PURIFIER_FILTER_RUNTIME: &str = "5902";
pub const ATTR_AIR_PURIFIER_FILTER_STATUS: &str = "5903";
pub const ATTR_AIR_PURIFIER_FILTER_LIFETIME_TOTAL: &str = "5904";
pub const ATTR_AIR_PURIFIER_CONTROLS_LOCKED: &str = "5905";
pub const ATTR_AIR_PURIFIER_LEDS_OFF: &str = "5906";
pub const ATTR_AIR_PURIFIER_AIR_QUALITY: &str = "5907";
pub const ATTR_AIR_PURIFIER_FAN_SPEED: &str = "5908";
pub const ATTR_AIR_PURIFIER_MOTOR_RUNTIME_TOTAL: &str = "5909";
pub const ATTR_AIR_PURIFIER_FILTER_LIFETIME_REMAINING: &str = "5910";
pub const AIR_PURIFIER_MODE_AUTO: u8 = 1;

pub const BRIGHTNESS_RANGE: (u16, u16) = (0, 254);
pub const MIRED_RANGE: (u16, u16) = (250, 454);
pub const HUE_RANGE: (u16, u16) = (0, 65_535);
pub const SATURATION_RANGE: (u16, u16) = (0, 65_279);
pub const XY_RANGE: (u16, u16) = (0, 65_535);
pub const BLIND_POSITION_RANGE: (u8, u8) = (0, 100);
pub const AIR_PURIFIER_FAN_RANGE: (u8, u8) = (2, 50);

#[must_use]
pub fn device_path(device_id: u32) -> String {
    format!("{ROOT_DEVICES}/{device_id}")
}

#[must_use]
pub fn group_path(group_id: u32) -> String {
    format!("{ROOT_GROUPS}/{group_id}")
}

#[must_use]
pub fn auth_path() -> String {
    format!("{ROOT_GATEWAY}/{ATTR_AUTH}")
}
