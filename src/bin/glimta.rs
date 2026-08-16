use std::{
    fs::OpenOptions,
    io::Write,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

use clap::{Args, Parser, Subcommand};
use glimta::{Credentials, Gateway, Result};

#[derive(Parser)]
#[command(name = "glimta", about = "Talk to a classic IKEA TRADFRI gateway")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Discover a gateway through mDNS.
    Discover,
    /// Provision long-lived credentials using the printed gateway security code.
    Provision {
        #[arg(long)]
        gateway: Option<String>,
        #[arg(long)]
        credentials: PathBuf,
        #[arg(long)]
        identity: Option<String>,
    },
    /// List devices.
    Devices(ConnectionArgs),
    /// List groups.
    Groups(ConnectionArgs),
    /// Turn a light on.
    LightOn {
        #[command(flatten)]
        connection: ConnectionArgs,
        #[arg(long)]
        device: u32,
    },
    /// Turn a light off.
    LightOff {
        #[command(flatten)]
        connection: ConnectionArgs,
        #[arg(long)]
        device: u32,
    },
    /// Set light brightness in the gateway range 0..=254.
    LightBrightness {
        #[command(flatten)]
        connection: ConnectionArgs,
        #[arg(long)]
        device: u32,
        #[arg(long)]
        value: u16,
    },
    /// Turn a switched outlet on or off.
    Socket {
        #[command(flatten)]
        connection: ConnectionArgs,
        #[arg(long)]
        device: u32,
        #[arg(long)]
        on: bool,
    },
    /// Set a blind position in percent.
    Blind {
        #[command(flatten)]
        connection: ConnectionArgs,
        #[arg(long)]
        device: u32,
        #[arg(long)]
        position: u8,
    },
    /// Put an air purifier in automatic mode.
    PurifierAuto {
        #[command(flatten)]
        connection: ConnectionArgs,
        #[arg(long)]
        device: u32,
    },
    /// Turn an air purifier off.
    PurifierOff {
        #[command(flatten)]
        connection: ConnectionArgs,
        #[arg(long)]
        device: u32,
    },
    /// Set air purifier fan speed in the gateway range 2..=50.
    PurifierFan {
        #[command(flatten)]
        connection: ConnectionArgs,
        #[arg(long)]
        device: u32,
        #[arg(long)]
        speed: u8,
    },
}

#[derive(Args)]
struct ConnectionArgs {
    /// Gateway IPv4/IPv6 address, optionally with port. Omit to use mDNS.
    #[arg(long)]
    gateway: Option<String>,
    /// JSON credential file previously created by `glimta provision`.
    #[arg(long)]
    credentials: PathBuf,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        CliCommand::Discover => discover().await?,
        CliCommand::Provision {
            gateway,
            credentials,
            identity,
        } => provision(gateway.as_deref(), &credentials, identity.as_deref()).await?,
        CliCommand::Devices(connection) => list_devices(&connection).await?,
        CliCommand::Groups(connection) => list_groups(&connection).await?,
        CliCommand::LightOn { connection, device } => {
            client(&connection)
                .await?
                .set_light_state(device, true)
                .await?;
        }
        CliCommand::LightOff { connection, device } => {
            client(&connection)
                .await?
                .set_light_state(device, false)
                .await?;
        }
        CliCommand::LightBrightness {
            connection,
            device,
            value,
        } => {
            client(&connection)
                .await?
                .set_light_brightness(device, value, None)
                .await?;
        }
        CliCommand::Socket {
            connection,
            device,
            on,
        } => {
            client(&connection)
                .await?
                .set_socket_state(device, on)
                .await?;
        }
        CliCommand::Blind {
            connection,
            device,
            position,
        } => {
            client(&connection)
                .await?
                .set_blind_position(device, position)
                .await?;
        }
        CliCommand::PurifierAuto { connection, device } => {
            client(&connection)
                .await?
                .set_air_purifier_auto(device)
                .await?;
        }
        CliCommand::PurifierOff { connection, device } => {
            client(&connection)
                .await?
                .turn_air_purifier_off(device)
                .await?;
        }
        CliCommand::PurifierFan {
            connection,
            device,
            speed,
        } => {
            client(&connection)
                .await?
                .set_air_purifier_fan_speed(device, speed)
                .await?;
        }
    }
    Ok(())
}

async fn discover() -> Result<()> {
    let gateway = Gateway::discover(Duration::from_secs(5)).await?;
    println!("{}", gateway.address());
    Ok(())
}

async fn provision(gateway: Option<&str>, path: &Path, identity: Option<&str>) -> Result<()> {
    let gateway = resolve_gateway(gateway).await?;
    let security_code = rpassword::prompt_password("Gateway security code: ")?;
    let credentials = if let Some(identity) = identity {
        gateway
            .provision_with_identity(&security_code, identity)
            .await?
    } else {
        gateway.provision(&security_code).await?
    };
    write_credentials(path, &credentials)?;
    println!("Credentials written to {}", path.display());
    Ok(())
}

async fn list_devices(connection: &ConnectionArgs) -> Result<()> {
    let (gateway, credentials) = load_connection(connection).await?;
    for device in gateway.connect(credentials).devices().await? {
        println!(
            "{}\t{}\t{:?}\treachable={:?}",
            device.id,
            device.name(),
            device.capabilities(),
            device.is_reachable()
        );
    }
    Ok(())
}

async fn list_groups(connection: &ConnectionArgs) -> Result<()> {
    let (gateway, credentials) = load_connection(connection).await?;
    for group in gateway.connect(credentials).groups().await? {
        println!(
            "{}\t{}\ton={}\tmembers={:?}",
            group.id,
            group.name(),
            group.is_on(),
            group.member_ids()
        );
    }
    Ok(())
}

async fn client(connection: &ConnectionArgs) -> Result<glimta::Client> {
    let (gateway, credentials) = load_connection(connection).await?;
    Ok(gateway.connect(credentials))
}

async fn load_connection(connection: &ConnectionArgs) -> Result<(Gateway, Credentials)> {
    let gateway = resolve_gateway(connection.gateway.as_deref()).await?;
    let credentials = read_credentials(&connection.credentials)?;
    Ok((gateway, credentials))
}

async fn resolve_gateway(value: Option<&str>) -> Result<Gateway> {
    match value {
        Some(value) => Ok(parse_gateway(value)?),
        None => Gateway::discover(Duration::from_secs(5)).await,
    }
}

fn parse_gateway(value: &str) -> std::io::Result<Gateway> {
    if let Ok(address) = value.parse::<SocketAddr>() {
        return Ok(Gateway::from_socket_addr(address));
    }
    value.parse::<IpAddr>().map(Gateway::new).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid gateway address {value:?}: {error}"),
        )
    })
}

fn read_credentials(path: &Path) -> Result<Credentials> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn write_credentials(path: &Path, credentials: &Credentials) -> Result<()> {
    let data = serde_json::to_vec_pretty(credentials)?;
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&data)?;
    file.write_all(b"\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
