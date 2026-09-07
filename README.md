# Glimta

A local-first Rust library and optional CLI for talking directly to the classic IKEA TRÅDFRI gateway.

Glimta uses the gateway's local CoAP/DTLS interface. It does not require IKEA cloud services, Home Assistant, MQTT, or a Python runtime, and it deliberately leaves deployment, credential storage, and automation policy to the application embedding it.

## What it supports

- deterministic mDNS discovery of one or more classic TRÅDFRI gateways
- first-time client provisioning from the printed gateway security code
- CoAP over DTLS-PSK using the gateway's compatible cipher suite
- device and group enumeration
- strict and best-effort bulk reads
- typed resources that tolerate unknown gateway attributes
- lights: on/off, brightness, color temperature, hex, XY, and HSB commands
- switched outlets
- blinds
- air purifiers
- groups
- bounded, cancellable CoAP Observe subscriptions exposed as async Rust streams
- opt-in resilient observations with bounded reconnect backoff
- an optional CLI for discovery, provisioning, inspection, and basic control

The protocol model is independent from network I/O, so parsing and command generation can be tested without physical hardware.

## Library usage

Enable the default `network` feature for discovery and gateway communication:

```toml
[dependencies]
glimta = { git = "https://github.com/Mik-pe/glimta" }
```

Provision credentials once and hand the returned value to your own credential store:

```rust,no_run
use std::time::Duration;

use glimta::Gateway;

# async fn example(security_code: &str) -> glimta::Result<()> {
let gateway = Gateway::discover(Duration::from_secs(5)).await?;
let credentials = gateway.provision(security_code).await?;

// Persist `credentials` using the embedding application's secret-storage policy.
let client = gateway.connect(credentials);

for device in client.devices().await? {
    println!("{}: {:?}", device.name(), device.capabilities());
}
# Ok(())
# }
```

If several gateways may exist, discover them all and choose explicitly. Results are de-duplicated and returned in deterministic hostname order:

```rust,no_run
use std::time::Duration;

use glimta::Gateway;

# async fn example() -> glimta::Result<()> {
let gateways = Gateway::discover_all(Duration::from_secs(5)).await?;
for gateway in gateways {
    println!("{:?}: {}", gateway.hostname(), gateway.address());
}
# Ok(())
# }
```

Connect later with previously provisioned credentials:

```rust,no_run
use std::{net::IpAddr, str::FromStr};

use glimta::{Credentials, Gateway};

# async fn example() -> glimta::Result<()> {
let gateway = Gateway::new(IpAddr::from_str("192.0.2.10").unwrap());
let credentials = Credentials::new("example-client", "example-pre-shared-key")?;
let client = gateway.connect(credentials);

client.set_light_state(65_537, true).await?;
client.set_light_brightness(65_537, 128, Some(10)).await?;
# Ok(())
# }
```

`192.0.2.10` and the credentials above are documentation-only placeholders.

### Best-effort reads

`devices()` and `groups()` remain strict: they return on the first resource that cannot be fetched or decoded. Applications that should keep healthy resources when one device is offline or malformed can use the best-effort variants:

```rust,no_run
# async fn example(client: glimta::Client) -> glimta::Result<()> {
let snapshot = client.devices_best_effort().await?;

for device in snapshot.items {
    println!("{}: {}", device.id, device.name());
}
for failure in snapshot.failures {
    eprintln!("device {} could not be read: {}", failure.id, failure.error);
}
# Ok(())
# }
```

Failure to enumerate the resource IDs or establish the shared read session still fails the call. Per-resource failures are isolated only after enumeration succeeds.

### Observe changes

A direct observation is cancellable and bounded. The channel keeps the newest snapshot instead of growing without limit. If the consumer falls behind, it receives `Error::ObservationLagged` and can then continue with the newest state.

```rust,no_run
# async fn example(client: glimta::Client, device_id: u32) -> glimta::Result<()> {
let mut updates = client.observe_device(device_id).await?;

while let Some(update) = updates.recv().await {
    match update {
        Ok(device) => println!("{} changed", device.name()),
        Err(glimta::Error::ObservationLagged { dropped }) => {
            eprintln!("skipped {dropped} stale update(s)");
        }
        Err(error) => return Err(error),
    }
}
# Ok(())
# }
```

Dropping an observation, or calling `cancel()`, sends an explicit CoAP Observe termination.

For long-lived consumers, resilient observation is opt-in. It creates a new DTLS/CoAP observation after transport loss or unexpected termination and retries with bounded exponential backoff. Transient failures remain visible as stream errors instead of being hidden:

```rust,no_run
use glimta::ReconnectOptions;

# async fn example(client: glimta::Client, device_id: u32) -> glimta::Result<()> {
let mut updates = client
    .observe_device_resilient(device_id, ReconnectOptions::default())
    .await?;

while let Some(update) = updates.recv().await {
    match update {
        Ok(device) => println!("{} changed", device.name()),
        Err(error) => eprintln!("observation is recovering: {error}"),
    }
}
# Ok(())
# }
```

## Core-only usage

Applications that only need protocol types and command construction can disable networking:

```toml
[dependencies]
glimta = { git = "https://github.com/Mik-pe/glimta", default-features = false }
```

That keeps CoAP, DTLS, Tokio, and mDNS out of the dependency graph.

## CLI

Build or install the optional CLI with the `cli` feature:

```bash
cargo run --features cli -- discover
cargo run --features cli -- provision --credentials ./glimta-credentials.json
cargo run --features cli -- devices --credentials ./glimta-credentials.json
```

`discover` prints every matching gateway in deterministic order. Device and group listing use best-effort reads, so a single broken resource is reported on stderr without hiding healthy entries.

The provisioning command reads the gateway security code without echoing it and writes the resulting credential file with owner-only permissions on Unix. Callers remain free to use a different secret store when using the library API.

A gateway address can be supplied explicitly instead of using mDNS:

```bash
cargo run --features cli -- devices \
  --gateway 192.0.2.10 \
  --credentials ./glimta-credentials.json
```

## Architecture

- `protocol` contains TRÅDFRI endpoint and attribute identifiers.
- `model` owns typed gateway resources while preserving unknown fields.
- `command` builds validated wire commands without doing I/O.
- `transport` owns CoAP over DTLS-PSK.
- `discovery` owns mDNS gateway discovery.
- `client` provides the async public API and Observe streams.

Glimta uses the CoAP crate without its bundled DTLS feature and supplies a small custom client transport over the maintained `dtls` crate. This keeps CoAP protocol handling independent from the concrete DTLS implementation and avoids pulling the CoAP crate's legacy DTLS dependency graph into Glimta.

Bulk reads reuse a read-only DTLS session. Writes open fresh sessions because classic gateways have historically behaved differently when multiple PUT operations reuse one connection.

## Compatibility references

Glimta is a new implementation informed by two existing open-source projects:

- `home-assistant-libs/pytradfri` for broad classic-gateway behaviour and resource coverage.
- `tirithen/tradfri_gateway` for prior Rust interoperability knowledge.

Neither project is required at runtime.

## Development

```bash
cargo fmt -- --check
cargo test --no-default-features --all-targets
cargo test --all-targets
cargo test --all-features --all-targets
cargo clippy --all-features --all-targets -- -D warnings
cargo generate-lockfile
cargo audit
```

CI tests core-only, default, and all-feature builds separately and audits the resolved dependency graph for RustSec advisories.

Real-gateway interoperability is intentionally separate from unit tests because CI does not assume access to local hardware.

## License

MIT
