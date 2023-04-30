pub mod esp;
pub mod govee;
mod keep_alive_job;

use esp::EspLed;
use govee::GoveeLed;

use async_trait::async_trait;
use bluer::{gatt::remote::Characteristic, AdapterEvent, Address, Device, Uuid};
use futures::{pin_mut, StreamExt};
use log::{debug, error, info};
use std::io::{self, Error, ErrorKind};

#[derive(Debug)]
pub enum Event {
    On,
    Off,
    Color(String),
    Other(Option<String>),
}

#[async_trait]
pub trait LedDevice {
    async fn connect(&mut self) -> io::Result<()>;

    async fn on_event(&mut self, event: Event) -> io::Result<()>;
}

#[derive(Debug, device_macro::Devices)]
pub enum Devices {
    Govee(GoveeLed),
    Esp(EspLed),
    Other(Other),
}

#[derive(Debug, Clone)]
pub struct Other {}

impl Other {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl LedDevice for Other {
    async fn connect(&mut self) -> io::Result<()> {
        Ok(())
    }

    async fn on_event(&mut self, event: Event) -> io::Result<()> {
        info!("Set led: {event:?}");
        Ok(())
    }
}

async fn discover_device(device_addr: Address) -> bluer::Result<Option<Device>> {
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;

    info!(
        "Discovering on Bluetooth adapter {} with address {}\n",
        adapter.name(),
        adapter.address().await?
    );

    let discover = adapter.discover_devices().await?;
    pin_mut!(discover);

    while let Some(evt) = discover.next().await {
        match evt {
            AdapterEvent::DeviceAdded(addr) => {
                let device = adapter.device(addr)?;
                let addr = device.address();

                info!("{}", addr);

                if addr == device_addr {
                    info!("Found led device on {device_addr}");

                    return Ok(Some(device));
                }
            }
            AdapterEvent::DeviceRemoved(_addr) => {
                // info!("Device removed {addr}");
            }
            _ => (),
        }
    }

    Ok(None)
}

const MAX_CONNECT_RETRIES: i32 = 2;

async fn connect_device(device: &Device) -> bluer::Result<()> {
    if device.is_connected().await? {
        return Ok(());
    }

    let mut retries = 0;
    loop {
        match device.connect().await {
            Ok(()) => break,
            Err(err) if retries <= MAX_CONNECT_RETRIES => {
                info!("Error while connecting to {}: {}", device.address(), &err);
                retries += 1;
            }
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

async fn find_characteristic(
    device: &Device,
    service_uuid: Uuid,
    characteristic_uuid: Uuid,
) -> bluer::Result<Option<Characteristic>> {
    let _uuids = device.uuids().await?.unwrap_or_default();

    for service in device.services().await? {
        let uuid = service.uuid().await?;
        if uuid == service_uuid {
            info!("Found service: {uuid}");

            for characteristic in service.characteristics().await? {
                let uuid = characteristic.uuid().await?;
                if uuid == characteristic_uuid {
                    info!("Found characteristic: {uuid}");

                    return Ok(Some(characteristic));
                }
            }
        }
    }

    Ok(None)
}
