use async_trait::async_trait;
use bluer::{gatt::remote::Characteristic, gatt::CharacteristicWriter, Address, Device, Uuid};
use log::{debug, error, info};
use std::io::{self, Error, ErrorKind};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

use super::{connect_device, discover_device, find_characteristic, Event, LedDevice};
use crate::keep_alive_job::KeepAlive;

#[derive(Debug)]
pub struct GoveeLed {
    addr: Address,
    service_uuid: Uuid,
    characteristic_uuid: Uuid,
    device: Option<Device>,
    characteristic: Option<Characteristic>,
    keep_alive: KeepAlive,
}

impl GoveeLed {
    pub fn new(addr: Address, service_uuid: Uuid, characteristic_uuid: Uuid) -> Self {
        let keep_alive = KeepAlive::new(Duration::from_secs(2));

        Self {
            addr,
            service_uuid,
            characteristic_uuid,
            device: None,
            characteristic: None,
            keep_alive,
        }
    }

    // 0x33, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x33
    async fn turn_on(&mut self) {
        let on_ev = vec![
            0x33, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x33,
        ];
        if let Some(characteristic) = &self.characteristic {
            characteristic.write(&on_ev).await.unwrap();
        }
    }

    // 0x33, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x32
    async fn turn_off(&mut self) {
        let off_ev = vec![
            0x33, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x32,
        ];
        if let Some(characteristic) = &self.characteristic {
            characteristic.write(&off_ev).await.unwrap();
        }
    }

    // https://github.com/egold555/Govee-Reverse-Engineering/blob/master/Products/H6127.md#set-color
    // 0x33, 0x05, 0x02, RED, GREEN, BLUE, 0x00, 0xFF, 0xAE, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, XOR
    // 0x33, 0x05, 0x02, RED, GREEN, BLUE, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, XOR
    async fn set_color(&mut self, color: String) {
        let mut color_ev = vec![0x33, 0x05, 0x02];

        let mut color_vals = color.chars().collect::<Vec<char>>();
        color_vals.remove(0);
        let rgb_colors = color_vals
            .chunks(2)
            .map(|c| c.iter().collect::<String>())
            .collect::<Vec<String>>();

        color_ev.push(u8::from_str_radix(&rgb_colors[0], 16).unwrap());
        color_ev.push(u8::from_str_radix(&rgb_colors[1], 16).unwrap());
        color_ev.push(u8::from_str_radix(&rgb_colors[2], 16).unwrap());

        // color_ev.extend([
        //     0x00, 0xFF, 0xAE, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        // ]);
        color_ev.extend([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);

        let mut xor = color_ev[0];
        for a in color_ev.iter().skip(1) {
            xor ^= a;
        }

        color_ev.push(xor);

        if let Some(characteristic) = &self.characteristic {
            characteristic.write(color_ev.as_slice()).await.unwrap();
        }
    }

    // 0x33, 0x04, BRIGHTNESS, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, (0x33 ^ 0x04 ^ BRIGHTNESS)
    async fn set_brightness(&mut self, brightness: u8) {
        let mut brightness_ev = vec![
            0x33, 0x04, brightness, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        brightness_ev.push(0x33 ^ 0x04 ^ brightness);

        if let Some(characteristic) = &self.characteristic {
            characteristic
                .write(brightness_ev.as_slice())
                .await
                .unwrap();
        }
    }
}

// TOOD: use anyhow error handling
#[async_trait]
impl LedDevice for GoveeLed {
    // TOOD: start job for sending `keep_alive` messages
    async fn connect(&mut self) -> io::Result<()> {
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

                    // Send keep alive packet every 2 seconds to ensure the connection remains established, until disconnected manually.
                    // 0xAA, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xAB
                    let keep_alive_ev = vec![
                        0xAA, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xAB,
                    ];
                    self.keep_alive.run(characteristic, keep_alive_ev);
                }
                Ok(None) => {
                    let err = Error::new(
                        ErrorKind::NotFound,
                        format!("Characteristic {} not found", self.characteristic_uuid),
                    );
                    return Err(err);
                }
                Err(e) => {
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

        match event {
            Event::On => self.turn_on().await,
            Event::Off => self.turn_off().await,
            Event::Color(color) => self.set_color(color).await,
            Event::Brightness(brightness) => self.set_brightness(brightness).await,
            Event::Other(_) => {}
        }

        Ok(())
    }
}
