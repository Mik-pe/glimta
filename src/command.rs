use serde_json::{Map, Value};
use thiserror::Error;

use crate::protocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Put,
    Post,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub method: Method,
    pub path: String,
    pub body: Option<Value>,
    pub observe: bool,
}

impl Command {
    #[must_use]
    pub fn get(path: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            path: path.into(),
            body: None,
            observe: false,
        }
    }

    #[must_use]
    pub fn put(path: impl Into<String>, body: Value) -> Self {
        Self {
            method: Method::Put,
            path: path.into(),
            body: Some(body),
            observe: false,
        }
    }

    #[must_use]
    pub fn post(path: impl Into<String>, body: Value) -> Self {
        Self {
            method: Method::Post,
            path: path.into(),
            body: Some(body),
            observe: false,
        }
    }

    #[must_use]
    pub fn observed(mut self) -> Self {
        self.observe = true;
        self
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandError {
    #[error("brightness {0} is outside the TRÅDFRI range 0..=254")]
    Brightness(u16),
    #[error("color temperature {0} is outside the TRÅDFRI range 250..=454 mired")]
    ColorTemperature(u16),
}

#[must_use]
pub fn list_devices() -> Command {
    Command::get(protocol::ROOT_DEVICES)
}

#[must_use]
pub fn get_device(device_id: u32) -> Command {
    Command::get(protocol::device_path(device_id))
}

#[must_use]
pub fn observe_device(device_id: u32) -> Command {
    Command::get(protocol::device_path(device_id)).observed()
}

/// Build the first-time credential provisioning request.
///
/// The transport layer must open this request using the gateway's printed
/// security code with the fixed `Client_identity` DTLS identity. The response
/// contains the long-lived PSK under attribute `9091`.
#[must_use]
pub fn provision_identity(identity: impl Into<String>) -> Command {
    let mut body = Map::new();
    body.insert(
        protocol::ATTR_CLIENT_IDENTITY.to_owned(),
        Value::String(identity.into()),
    );
    Command::post(protocol::auth_path(), Value::Object(body))
}

#[must_use]
pub fn set_light_state(device_id: u32, on: bool) -> Command {
    light_put(
        device_id,
        [(protocol::ATTR_DEVICE_STATE, Value::from(u8::from(on)))],
    )
}

/// Build a light brightness write.
///
/// `transition_time` is expressed in tenths of a second, matching the gateway
/// protocol.
///
/// # Errors
///
/// Returns [`CommandError::Brightness`] when `brightness` is outside the
/// gateway's accepted `0..=254` range.
pub fn set_light_brightness(
    device_id: u32,
    brightness: u16,
    transition_time: Option<u16>,
) -> Result<Command, CommandError> {
    if !(protocol::BRIGHTNESS_RANGE.0..=protocol::BRIGHTNESS_RANGE.1).contains(&brightness) {
        return Err(CommandError::Brightness(brightness));
    }

    let mut values = vec![(protocol::ATTR_LIGHT_DIMMER, Value::from(brightness))];
    if let Some(transition_time) = transition_time {
        values.push((protocol::ATTR_TRANSITION_TIME, Value::from(transition_time)));
    }

    Ok(light_put(device_id, values))
}

/// Build a white-spectrum color-temperature write.
///
/// `transition_time` is expressed in tenths of a second, matching the gateway
/// protocol.
///
/// # Errors
///
/// Returns [`CommandError::ColorTemperature`] when `mireds` is outside the
/// classic IKEA bulb range of `250..=454`.
pub fn set_light_color_temperature(
    device_id: u32,
    mireds: u16,
    transition_time: Option<u16>,
) -> Result<Command, CommandError> {
    if !(protocol::MIRED_RANGE.0..=protocol::MIRED_RANGE.1).contains(&mireds) {
        return Err(CommandError::ColorTemperature(mireds));
    }

    let mut values = vec![(protocol::ATTR_LIGHT_MIREDS, Value::from(mireds))];
    if let Some(transition_time) = transition_time {
        values.push((protocol::ATTR_TRANSITION_TIME, Value::from(transition_time)));
    }

    Ok(light_put(device_id, values))
}

fn light_put<I>(device_id: u32, values: I) -> Command
where
    I: IntoIterator<Item = (&'static str, Value)>,
{
    let light = values
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect::<Map<String, Value>>();

    let mut body = Map::new();
    body.insert(
        protocol::ATTR_LIGHT_CONTROL.to_owned(),
        Value::Array(vec![Value::Object(light)]),
    );

    Command::put(protocol::device_path(device_id), Value::Object(body))
}
