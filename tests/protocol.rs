use glimta::{
    Device, DeviceKind,
    command::{
        CommandError, Method, observe_device, provision_identity, set_light_brightness,
        set_light_state,
    },
};

#[test]
fn parses_a_classic_tradfri_light_payload() {
    let raw = r#"{
        "9003": 65537,
        "9001": "Vardagsrum",
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
    assert_eq!(device.name(), "Vardagsrum");
    assert_eq!(device.kind(), DeviceKind::Light);
    assert_eq!(device.is_reachable(), Some(true));
    assert_eq!(device.info.manufacturer.as_deref(), Some("IKEA of Sweden"));

    let light = device.primary_light().expect("light should exist");
    assert!(light.is_on());
    assert_eq!(light.brightness, Some(128));
    assert_eq!(light.color_temperature_mireds, Some(370));
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
fn rejects_invalid_brightness_before_network_io() {
    assert_eq!(
        set_light_brightness(65537, 255, None),
        Err(CommandError::Brightness(255))
    );
}

#[test]
fn observation_is_a_property_of_the_command_not_the_model() {
    let command = observe_device(65537);
    assert_eq!(command.method, Method::Get);
    assert!(command.observe);
}
