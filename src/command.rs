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
    #[error("brightness {0} is outside the TRADFRI range 0..=254")]
    Brightness(u16),
    #[error("color temperature {0} is outside the TRADFRI range 250..=454 mired")]
    ColorTemperature(u16),
    #[error("hue {0} is outside the TRADFRI range 0..=65535")]
    Hue(u16),
    #[error("saturation {0} is outside the TRADFRI range 0..=65279")]
    Saturation(u16),
    #[error("x color coordinate {0} is outside the TRADFRI range 0..=65535")]
    ColorX(u16),
    #[error("y color coordinate {0} is outside the TRADFRI range 0..=65535")]
    ColorY(u16),
    #[error("blind position {0} is outside the TRADFRI range 0..=100")]
    BlindPosition(u8),
    #[error("air purifier fan speed {0} is outside the TRADFRI range 2..=50")]
    AirPurifierFanSpeed(u8),
    #[error("light color must be exactly six hexadecimal digits, got {0:?}")]
    HexColor(String),
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

#[must_use]
pub fn list_groups() -> Command {
    Command::get(protocol::ROOT_GROUPS)
}

#[must_use]
pub fn get_group(group_id: u32) -> Command {
    Command::get(protocol::group_path(group_id))
}

#[must_use]
pub fn observe_group(group_id: u32) -> Command {
    Command::get(protocol::group_path(group_id)).observed()
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
    endpoint_put(
        protocol::device_path(device_id),
        protocol::ATTR_LIGHT_CONTROL,
        [(protocol::ATTR_DEVICE_STATE, Value::from(u8::from(on)))],
    )
}

/// Build a light brightness command.
///
/// # Errors
///
/// Returns an error when `brightness` is outside `0..=254`.
pub fn set_light_brightness(
    device_id: u32,
    brightness: u16,
    transition_time: Option<u16>,
) -> Result<Command, CommandError> {
    validate_u16(
        brightness,
        protocol::BRIGHTNESS_RANGE,
        CommandError::Brightness,
    )?;
    let mut values = vec![(protocol::ATTR_LIGHT_DIMMER, Value::from(brightness))];
    push_transition(&mut values, transition_time);
    Ok(endpoint_put(
        protocol::device_path(device_id),
        protocol::ATTR_LIGHT_CONTROL,
        values,
    ))
}

/// Build a light color-temperature command.
///
/// # Errors
///
/// Returns an error when `mireds` is outside `250..=454`.
pub fn set_light_color_temperature(
    device_id: u32,
    mireds: u16,
    transition_time: Option<u16>,
) -> Result<Command, CommandError> {
    validate_u16(
        mireds,
        protocol::MIRED_RANGE,
        CommandError::ColorTemperature,
    )?;
    let mut values = vec![(protocol::ATTR_LIGHT_MIREDS, Value::from(mireds))];
    push_transition(&mut values, transition_time);
    Ok(endpoint_put(
        protocol::device_path(device_id),
        protocol::ATTR_LIGHT_CONTROL,
        values,
    ))
}

/// Build a light hexadecimal-color command.
///
/// # Errors
///
/// Returns an error unless `color` contains exactly six hexadecimal digits.
pub fn set_light_hex_color(
    device_id: u32,
    color: &str,
    transition_time: Option<u16>,
) -> Result<Command, CommandError> {
    let color = normalize_hex_color(color)?;
    let mut values = vec![(protocol::ATTR_LIGHT_COLOR_HEX, Value::from(color))];
    push_transition(&mut values, transition_time);
    Ok(endpoint_put(
        protocol::device_path(device_id),
        protocol::ATTR_LIGHT_CONTROL,
        values,
    ))
}

/// Build a light XY-color command.
///
/// # Errors
///
/// Returns an error when either coordinate is outside the gateway range.
pub fn set_light_xy_color(
    device_id: u32,
    x: u16,
    y: u16,
    transition_time: Option<u16>,
) -> Result<Command, CommandError> {
    validate_u16(x, protocol::XY_RANGE, CommandError::ColorX)?;
    validate_u16(y, protocol::XY_RANGE, CommandError::ColorY)?;
    let mut values = vec![
        (protocol::ATTR_LIGHT_COLOR_X, Value::from(x)),
        (protocol::ATTR_LIGHT_COLOR_Y, Value::from(y)),
    ];
    push_transition(&mut values, transition_time);
    Ok(endpoint_put(
        protocol::device_path(device_id),
        protocol::ATTR_LIGHT_CONTROL,
        values,
    ))
}

/// Build a light HSB command.
///
/// # Errors
///
/// Returns an error when hue, saturation, or optional brightness is outside
/// its gateway range.
pub fn set_light_hsb(
    device_id: u32,
    hue: u16,
    saturation: u16,
    brightness: Option<u16>,
    transition_time: Option<u16>,
) -> Result<Command, CommandError> {
    validate_u16(hue, protocol::HUE_RANGE, CommandError::Hue)?;
    validate_u16(
        saturation,
        protocol::SATURATION_RANGE,
        CommandError::Saturation,
    )?;
    let mut values = vec![
        (protocol::ATTR_LIGHT_COLOR_HUE, Value::from(hue)),
        (
            protocol::ATTR_LIGHT_COLOR_SATURATION,
            Value::from(saturation),
        ),
    ];
    if let Some(brightness) = brightness {
        validate_u16(
            brightness,
            protocol::BRIGHTNESS_RANGE,
            CommandError::Brightness,
        )?;
        values.push((protocol::ATTR_LIGHT_DIMMER, Value::from(brightness)));
    }
    push_transition(&mut values, transition_time);
    Ok(endpoint_put(
        protocol::device_path(device_id),
        protocol::ATTR_LIGHT_CONTROL,
        values,
    ))
}

#[must_use]
pub fn set_socket_state(device_id: u32, on: bool) -> Command {
    endpoint_put(
        protocol::device_path(device_id),
        protocol::ATTR_SOCKET_CONTROL,
        [(protocol::ATTR_DEVICE_STATE, Value::from(u8::from(on)))],
    )
}

/// Build a blind-position command.
///
/// # Errors
///
/// Returns an error when `position` is outside `0..=100`.
pub fn set_blind_position(device_id: u32, position: u8) -> Result<Command, CommandError> {
    if !(protocol::BLIND_POSITION_RANGE.0..=protocol::BLIND_POSITION_RANGE.1).contains(&position) {
        return Err(CommandError::BlindPosition(position));
    }
    Ok(endpoint_put(
        protocol::device_path(device_id),
        protocol::ROOT_BLINDS,
        [(protocol::ATTR_BLIND_CURRENT_POSITION, Value::from(position))],
    ))
}

#[must_use]
pub fn trigger_blind(device_id: u32) -> Command {
    endpoint_put(
        protocol::device_path(device_id),
        protocol::ROOT_BLINDS,
        [(protocol::ATTR_BLIND_TRIGGER, Value::Bool(true))],
    )
}

#[must_use]
pub fn turn_air_purifier_off(device_id: u32) -> Command {
    air_purifier_put(device_id, protocol::ATTR_AIR_PURIFIER_MODE, Value::from(0))
}

#[must_use]
pub fn set_air_purifier_auto(device_id: u32) -> Command {
    air_purifier_put(
        device_id,
        protocol::ATTR_AIR_PURIFIER_MODE,
        Value::from(protocol::AIR_PURIFIER_MODE_AUTO),
    )
}

/// Build an air-purifier fan-speed command.
///
/// # Errors
///
/// Returns an error when `speed` is outside `2..=50`.
pub fn set_air_purifier_fan_speed(
    device_id: u32,
    speed: u8,
) -> Result<Command, CommandError> {
    if !(protocol::AIR_PURIFIER_FAN_RANGE.0..=protocol::AIR_PURIFIER_FAN_RANGE.1).contains(&speed) {
        return Err(CommandError::AirPurifierFanSpeed(speed));
    }
    Ok(air_purifier_put(
        device_id,
        protocol::ATTR_AIR_PURIFIER_MODE,
        Value::from(speed),
    ))
}

#[must_use]
pub fn set_air_purifier_controls_locked(device_id: u32, locked: bool) -> Command {
    air_purifier_put(
        device_id,
        protocol::ATTR_AIR_PURIFIER_CONTROLS_LOCKED,
        Value::from(u8::from(locked)),
    )
}

#[must_use]
pub fn set_air_purifier_leds_off(device_id: u32, leds_off: bool) -> Command {
    air_purifier_put(
        device_id,
        protocol::ATTR_AIR_PURIFIER_LEDS_OFF,
        Value::from(u8::from(leds_off)),
    )
}

#[must_use]
pub fn set_group_state(group_id: u32, on: bool) -> Command {
    direct_put(
        protocol::group_path(group_id),
        [(protocol::ATTR_DEVICE_STATE, Value::from(u8::from(on)))],
    )
}

/// Build a group brightness command.
///
/// # Errors
///
/// Returns an error when `brightness` is outside `0..=254`.
pub fn set_group_brightness(
    group_id: u32,
    brightness: u16,
    transition_time: Option<u16>,
) -> Result<Command, CommandError> {
    validate_u16(
        brightness,
        protocol::BRIGHTNESS_RANGE,
        CommandError::Brightness,
    )?;
    let mut values = vec![(protocol::ATTR_LIGHT_DIMMER, Value::from(brightness))];
    push_transition(&mut values, transition_time);
    Ok(direct_put(protocol::group_path(group_id), values))
}

/// Build a group color-temperature command.
///
/// # Errors
///
/// Returns an error when `mireds` is outside `250..=454`.
pub fn set_group_color_temperature(
    group_id: u32,
    mireds: u16,
    transition_time: Option<u16>,
) -> Result<Command, CommandError> {
    validate_u16(
        mireds,
        protocol::MIRED_RANGE,
        CommandError::ColorTemperature,
    )?;
    let mut values = vec![(protocol::ATTR_LIGHT_MIREDS, Value::from(mireds))];
    push_transition(&mut values, transition_time);
    Ok(direct_put(protocol::group_path(group_id), values))
}

fn air_purifier_put(device_id: u32, key: &'static str, value: Value) -> Command {
    endpoint_put(
        protocol::device_path(device_id),
        protocol::ROOT_AIR_PURIFIER,
        [(key, value)],
    )
}

fn endpoint_put<I>(path: String, endpoint: &'static str, values: I) -> Command
where
    I: IntoIterator<Item = (&'static str, Value)>,
{
    let endpoint_values = object(values);
    let mut body = Map::new();
    body.insert(
        endpoint.to_owned(),
        Value::Array(vec![Value::Object(endpoint_values)]),
    );
    Command::put(path, Value::Object(body))
}

fn direct_put<I>(path: String, values: I) -> Command
where
    I: IntoIterator<Item = (&'static str, Value)>,
{
    Command::put(path, Value::Object(object(values)))
}

fn object<I>(values: I) -> Map<String, Value>
where
    I: IntoIterator<Item = (&'static str, Value)>,
{
    values
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

fn push_transition(values: &mut Vec<(&'static str, Value)>, transition_time: Option<u16>) {
    if let Some(transition_time) = transition_time {
        values.push((protocol::ATTR_TRANSITION_TIME, Value::from(transition_time)));
    }
}

fn validate_u16(
    value: u16,
    range: (u16, u16),
    error: fn(u16) -> CommandError,
) -> Result<(), CommandError> {
    if (range.0..=range.1).contains(&value) {
        Ok(())
    } else {
        Err(error(value))
    }
}

fn normalize_hex_color(color: &str) -> Result<String, CommandError> {
    let normalized = color.strip_prefix('#').unwrap_or(color);
    if normalized.len() == 6 && normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(normalized.to_ascii_lowercase())
    } else {
        Err(CommandError::HexColor(color.to_owned()))
    }
}
