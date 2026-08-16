# Glimta agent notes

Glimta is a Rust implementation for the classic IKEA TRÅDFRI gateway.

## Direction

- Keep the project local-first. No IKEA cloud dependency is required for core operation.
- Prefer idiomatic Rust APIs over a mechanical translation of pytradfri classes.
- Treat `home-assistant-libs/pytradfri` as the broad behaviour reference and `tirithen/tradfri_gateway` as an interoperability reference.
- Preserve unknown numeric gateway fields where practical. Real gateways and third-party Zigbee devices can expose odd payloads.
- Keep protocol modelling independent from CoAP/DTLS I/O so fixtures can exercise parsing and command generation without hardware.
- Do not introduce Home Assistant, MQTT, or a particular automation daemon into the core crate. Those belong in consumers or adapters built on top of Glimta.

## Public project boundary

Glimta is a public, reusable library and CLI. Keep its source, documentation, examples, tests, issue descriptions, and checked-in configuration independent from any contributor's private deployment.

- Never document real hostnames, domains, IP addresses, machine names, credential paths, network layout, or deployment topology from a contributor's home or production environment.
- Never encode assumptions about where Glimta runs or which application embeds it.
- Use neutral placeholders and loopback/documentation addresses in examples.
- Let callers choose credential storage, process supervision, networking, MQTT topics, and deployment layout.
- Never log gateway security codes, provisioned PSKs, or raw credential-bearing packets.

## Gateway support

The primary compatibility target is the classic TRÅDFRI gateway. The library should support its useful local resources without requiring a cloud service:

1. Discover `TRADFRI-Gateway-*` over `_coap._udp.local.`.
2. Provision a client identity through `15011/9063` using the printed gateway security code.
3. Return the long-lived identity/PSK pair to the caller without choosing persistence policy.
4. List, fetch, and observe devices and groups.
5. Model and control lights, switched outlets, blinds, air purifiers, signal repeaters, and groups where the gateway exposes those capabilities.
6. Surface observations as cancellable async Rust streams.

## Protocol constraints

- Classic TRÅDFRI uses CoAP over DTLS on UDP port 5684.
- Credential provisioning starts with DTLS identity `Client_identity` and the printed security code, then uses the returned long-lived identity/PSK pair.
- Isolate writes onto fresh DTLS sessions unless interoperability testing proves connection reuse safe; classic gateways have historically behaved differently for repeated PUTs than for GETs.
- Validate command ranges before network I/O.
- Add fixture tests for every newly supported device type before adding higher-level control helpers.
- Unknown numeric attributes must not make otherwise useful resources fail to deserialize.

## Quality bar

Before merging Rust changes, run:

```bash
cargo fmt -- --check
cargo test --no-default-features --all-targets
cargo test --all-features --all-targets
cargo clippy --all-features --all-targets -- -D warnings
```

Public API changes should include tests and a short README update when user-visible behaviour changes.
