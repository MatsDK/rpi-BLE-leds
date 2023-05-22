use async_trait::async_trait;
use bluer::{gatt::remote::Characteristic, Address, Device, Uuid};
use log::{debug, error, info};
use std::io::{self, Error, ErrorKind};

use super::{connect_device, discover_device, find_characteristic, Event, LedDevice};

#[derive(Debug, Clone)]
pub struct EspLed {
    addr: Address,
    service_uuid: Uuid,
    characteristic_uuid: Uuid,
    device: Option<Device>,
    characteristic: Option<Characteristic>,
}

impl EspLed {
    pub fn new(addr: Address, service_uuid: Uuid, characteristic_uuid: Uuid) -> Self {
        Self {
            addr,
            service_uuid,
            characteristic_uuid,
            device: None,
            characteristic: None,
        }
    }
}

// TOOD: use anyhow error handling
#[async_trait]
impl LedDevice for EspLed {
    async fn connect(&mut self) -> io::Result<()> {
        info!("Start connect to esp");

        Ok(())
    }

    async fn disconnect(&mut self) -> io::Result<()> {
        Ok(())
    }

    async fn on_event(&mut self, event: Event) -> io::Result<()> {
        info!("Set led on {:?}, {:?}", self.addr, event);

        Ok(())
    }
}
