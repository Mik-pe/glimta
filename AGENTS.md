# Glimta agent notes

Glimta is a Rust implementation for the classic IKEA TRÅDFRI gateway.

## Direction

- Keep the project local-first. No IKEA cloud dependency is required for core operation.
- Prefer idiomatic Rust APIs over a mechanical translation of pytradfri classes.
- Treat `home-assistant-libs/pytradfri` as the broad behaviour reference and `tirithen/tradfri_gateway` as an interoperability reference.
- Preserve unknown numeric gateway fields where practical. Real gateways and third-party Zigbee devices can expose odd payloads.
- Keep protocol modelling independent from CoAP/DTLS I/O so fixtures can exercise parsing and command generation without hardware.
- Do not introduce Home Assistant or MQTT into the core crate. Those belong in adapters built on top of Glimta.

## First vertical slice

1. Discover `TRADFRI-Gateway-*` over `_coap._udp.local.`.
2. Provision a client identity through `15011/9063` using the printed gateway security code.
3. Persist the returned PSK outside the repository.
4. List device IDs from `15001` and fetch individual devices.
5. Support light on/off and brightness writes.
6. Observe a device and surface state changes as an async stream.

## Protocol constraints

- Classic TRÅDFRI uses CoAP over DTLS on UDP port 5684.
- Credential provisioning starts with DTLS identity `Client_identity` and the printed security code, then stores the returned long-lived identity/PSK pair.
- Never log gateway security codes, provisioned PSKs, or raw credential-bearing packets.
- Validate command ranges before network I/O.
- Add fixture tests for every newly supported device type before adding higher-level control helpers.

## Quality bar

Before committing Rust changes, run:

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Public API changes should include tests and a short README update when user-visible behaviour changes.
