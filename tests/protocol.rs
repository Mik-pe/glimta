use glimta::{
    Credentials, Device, DeviceKind, Group,
    command::{
        CommandError, Method, observe_device, provision_identity, set_air_purifier_auto,
        set_air_purifier_fan_speed, set_blind_position, set_group_brightness, set_group_state,
        set_light_brightness, set_light_state, set_socket_state,
    },
};

#[test]
fn parses_a_classic_tradfri_light_payload() {
    let raw = r#"{
        "9003": 65537,
        "9001": "Living room",
        "5750": 2,
        "9019": 1,
        "9020": 1700000000,
        "3": {
            "0": "IKEA of Sweden",
            "1": "TRADFRI bulb E27",
            "2": "deadbeef",
            "3": "1.2.3"
        },
        "3311": [{
            "9003": 0,
            "5850": 1,
            "5851": 128,
            "5711": 370
        }]
    }"#;

    let device: Device = serde_json::from_str(raw).expect("fixture should deserialize");

    assert_eq!(device.id, 65537);
    assert_eq!(device.name(), "Living room");
    assert_eq!(device.kind(), DeviceKind::Light);
    assert_eq!(device.is_reachable(), Some(true));
    assert_eq!(device.info.manufacturer.as_deref(), Some("IKEA of Sweden"));

    let light = device.primary_light().expect("light should exist");
    assert!(light.is_on());
    assert_eq!(light.brightness, Some(128));
    assert_eq!(light.color_temperature_mireds, Some(370));
}

#[test]
fn parses_multiple_device_capabilities_and_preserves_unknown_fields() {
    let raw = r#"{
        "9003": 42,
        "9001": "Mixed fixture",
        "3": {"0": "IKEA of Sweden", "1": "fixture", "7777": "future-info"},
        "3312": [{"9003": 0, "5850": 1}],
        "15015": [{"9003": 1, "5536": 47.5}],
        "15025": [{
            "9003": 2,
            "5900": 1,
            "5905": 1,
            "5906": 0,
            "5907": 23,
            "5908": 17,
            "5903": 0
        }],
        "9999": {"unknown": true}
    }"#;

    let device: Device = serde_json::from_str(raw).expect("fixture should deserialize");
    assert_eq!(
        device.capabilities(),
        vec![
            DeviceKind::Socket,
            DeviceKind::Blind,
            DeviceKind::AirPurifier
        ]
    );
    assert!(device.sockets[0].is_on());
    assert_eq!(device.blinds[0].current_position, 47.5);
    assert_eq!(device.air_purifiers[0].is_auto_mode(), Some(true));
    assert_eq!(device.air_purifiers[0].controls_locked(), Some(true));
    assert_eq!(device.air_purifiers[0].leds_off(), Some(false));
    assert!(device.extra.contains_key("9999"));
    assert!(device.info.extra.contains_key("7777"));
}

#[test]
fn parses_group_members() {
    let raw = r#"{
        "9003": 12,
        "9001": "Downstairs",
        "5850": 1,
        "5851": 180,
        "9018": {"15002": {"9003": [65536, 65537]}}
    }"#;

    let group: Group = serde_json::from_str(raw).expect("group fixture should deserialize");
    assert!(group.is_on());
    assert_eq!(group.member_ids(), vec![65536, 65537]);
}

#[test]
fn builds_the_gateway_identity_provisioning_request() {
    let command = provision_identity("glimta-test");

    assert_eq!(command.method, Method::Post);
    assert_eq!(command.path, "15011/9063");
    assert_eq!(command.body.expect("body")["9090"], "glimta-test");
}

#[test]
fn builds_light_commands_using_the_gateway_wire_shape() {
    let command = set_light_state(65537, true);

    assert_eq!(command.method, Method::Put);
    assert_eq!(command.path, "15001/65537");
    assert_eq!(command.body.expect("body")["3311"][0]["5850"], 1);

    let command = set_light_brightness(65537, 42, Some(10)).expect("valid brightness");
    let body = command.body.expect("body");
    assert_eq!(body["3311"][0]["5851"], 42);
    assert_eq!(body["3311"][0]["5712"], 10);
}

#[test]
fn builds_socket_blind_purifier_and_group_commands() {
    let socket = set_socket_state(7, true).body.expect("socket body");
    assert_eq!(socket["3312"][0]["5850"], 1);

    let blind = set_blind_position(8, 75)
        .expect("valid position")
        .body
        .expect("blind body");
    assert_eq!(blind["15015"][0]["5536"], 75);

    let purifier = set_air_purifier_auto(9).body.expect("purifier body");
    assert_eq!(purifier["15025"][0]["5900"], 1);

    let group = set_group_state(10, false).body.expect("group body");
    assert_eq!(group["5850"], 0);

    let group = set_group_brightness(10, 99, Some(5))
        .expect("valid group brightness")
        .body
        .expect("group brightness body");
    assert_eq!(group["5851"], 99);
    assert_eq!(group["5712"], 5);
}

#[test]
fn validates_device_ranges_before_network_io() {
    assert_eq!(
        set_light_brightness(65537, 255, None),
        Err(CommandError::Brightness(255))
    );
    assert_eq!(
        set_blind_position(1, 101),
        Err(CommandError::BlindPosition(101))
    );
    assert_eq!(
        set_air_purifier_fan_speed(1, 51),
        Err(CommandError::AirPurifierFanSpeed(51))
    );
}

#[test]
fn observation_is_a_property_of_the_command_not_the_model() {
    let command = observe_device(65537);
    assert_eq!(command.method, Method::Get);
    assert!(command.observe);
}

#[test]
fn credentials_never_debug_the_secret() {
    let credentials = Credentials::new("client", "super-secret").expect("valid credentials");
    let debug = format!("{credentials:?}");
    assert!(debug.contains("client"));
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("super-secret"));
}
