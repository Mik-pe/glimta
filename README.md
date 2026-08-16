# Glimta

A small Rust library and CLI for talking directly to the classic IKEA TRÅDFRI gateway over the local network.

Glimta is intentionally local-first. The first target is the old TRÅDFRI gateway, using its CoAP/DTLS interface, without depending on IKEA cloud services, Home Assistant, or a Python runtime.

> Early development: the protocol types and command model are being built first. Network transport and real-gateway integration are next.

## Goals

- discover a TRÅDFRI gateway on the LAN
- provision a client identity from the gateway security code
- communicate over CoAP + DTLS PSK
- list and observe devices and groups
- control lights, outlets, blinds, and other supported devices
- expose a small idiomatic async Rust API suitable for home automation daemons
- keep the protocol layer testable without physical hardware

## First vertical slice

```text
find gateway -> authenticate -> list devices -> inspect light -> on/off/dim -> observe state
```

## Planned API

```rust,ignore
let gateway = glimta::Gateway::discover().await?;
let session = gateway.authenticate(security_code).await?;

for device in session.devices().await? {
    println!("{}: {:?}", device.name(), device.kind());
}

session.light(light_id).set_brightness(42).await?;
```

## Architecture

The crate is being split by responsibility rather than mirroring pytradfri class-for-class:

- `protocol` contains TRÅDFRI endpoint and attribute identifiers.
- `model` owns typed representations of gateway resources.
- `command` builds protocol writes without doing any I/O.
- the upcoming transport layer will own CoAP, DTLS, discovery, retries, and observation.

This makes captured gateway payloads usable as fixtures and keeps the wire protocol independent from whichever async transport implementation Glimta settles on.

## Protocol references

Glimta is informed by the behaviour documented and exercised by:

- `home-assistant-libs/pytradfri`, used as the broad behaviour reference.
- `tirithen/tradfri_gateway`, used as a Rust/TRÅDFRI interoperability reference.

Glimta is a new implementation and does not require either project at runtime.

## Status

The classic TRÅDFRI gateway is discontinued hardware. Glimta exists to keep useful hardware useful and to provide a clean Rust building block for local home automation.

## License

MIT
