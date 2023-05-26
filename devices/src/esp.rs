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

        if let Some(device) = &self.device {
            if device.is_connected().await? {
                info!("Device already connected");
                return Ok(());
            }
        }

        match discover_device(self.addr).await {
            Ok(Some(device)) => {
                self.device = Some(device);
            }
            Ok(None) => {
                let err = Error::new(
                    ErrorKind::NotFound,
                    format!("Device {} not found", self.addr),
                );
                return Err(err);
            }
            Err(e) => {
                let err = Error::new(
                    ErrorKind::NotFound,
                    format!("Error searching for {}: {e}", self.addr),
                );
                return Err(err);
            }
        }

        if let Some(device) = &self.device {
            match connect_device(device).await {
                Ok(()) => {}
                Err(e) => {
                    let err = Error::new(
                        ErrorKind::NotFound,
                        format!("Error connecting to {}: {e}", self.addr),
                    );
                    return Err(err);
                }
            }

            info!("Successfully connected to {:?}", self.device);

            match find_characteristic(device, self.service_uuid, self.characteristic_uuid).await {
                Ok(Some(characteristic)) => {
                    self.characteristic = Some(characteristic.clone());
                }
                Ok(None) => {
                    let err = Error::new(
                        ErrorKind::NotFound,
                        format!("Characteristic {} not found", self.characteristic_uuid),
                    );
                    return Err(err);
                }
                Err(e) => {
                    error!("{e}");
                    let err = Error::new(
                        ErrorKind::NotFound,
                        format!(
                            "Error searching for characteristic {}: {e}",
                            self.characteristic_uuid
                        ),
                    );
                    return Err(err);
                }
            }
            info!("successfully found char");
        }

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
