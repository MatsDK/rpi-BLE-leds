use bluer::{gatt::remote::Characteristic, AdapterEvent, Address, Device, Result, Uuid};
use futures::{pin_mut, StreamExt};
use log::{debug, error, info, log_enabled, Level};
use std::env;
use std::str::FromStr;
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::sleep,
};

use tower::ServiceExt;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone)]
struct LedDevice {
    device: Device,
    characteristic: Option<Characteristic>,
}

impl LedDevice {
    async fn set_charasteristic(&mut self) -> bluer::Result<()> {
        if !self.device.is_connected().await? {
            info!("Connecting device");

            let mut retries = 2;
            loop {
                match self.device.connect().await {
                    Ok(()) => break,
                    Err(err) if retries > 0 => {
                        info!("    Connect error: {}", &err);
                        retries -= 1;
                    }
                    Err(err) => return Err(err),
                }
            }
            info!("Connected");
        }

        let led_service_uuid = Uuid::parse_str("000102030405060708090a0b0c0d1910").unwrap();
        let led_char_uuid = Uuid::parse_str("000102030405060708090a0b0c0d2b11").unwrap();

        for service in self.device.services().await? {
            let uuid = service.uuid().await?;
            if led_service_uuid == uuid {
                debug!(">> Found service with uuid: {:?}", uuid);
                for characteristic in service.characteristics().await? {
                    let uuid = characteristic.uuid().await?;

                    if uuid == led_char_uuid {
                        debug!(">> Found our characteristic {}", uuid);

                        let flags = characteristic.flags().await?;
                        debug!(">> Characteristic UUID: {} Flags: {:?}", &uuid, flags);

                        self.characteristic = Some(characteristic);
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn turn_on(&mut self) {
        self.set_charasteristic().await.unwrap();
        let on_ev = vec![
            0x33, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x33,
        ];
        if let Some(characteristic) = &self.characteristic {
            characteristic.write(&on_ev).await.unwrap();
        }
    }

    async fn turn_off(&mut self) {
        self.set_charasteristic().await.unwrap();
        let off_ev = vec![
            0x33, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x32,
        ];
        if let Some(characteristic) = &self.characteristic {
            characteristic.write(&off_ev).await.unwrap();
        }
    }

    async fn set_color(&mut self, color: String) {
        self.set_charasteristic().await.unwrap();

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

        color_ev.extend([
            0x00, 0xFF, 0xAE, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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
}

async fn connect_device() -> bluer::Result<Option<LedDevice>> {
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;

    let led_addr = Address::from_str("A4:C1:38:EC:91:32")?;

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
                info!("Device {:?} on {:?}", device.alias().await, addr);
                let props = device.all_properties().await?;
                for prop in props {
                    info!("    {:?}", &prop);
                }
                if addr == led_addr {
                    let uuids = device.uuids().await?.unwrap_or_default();

                    info!(
                        "Discovered led device {} with service UUIDs {:?}",
                        addr, &uuids
                    );

                    return Ok(Some(LedDevice {
                        characteristic: None,
                        device,
                    }));

                    info!("Disconnecting");
                    match device.disconnect().await {
                        Ok(()) => break,
                        Err(err) => {}
                    }
                }
            }
            AdapterEvent::DeviceRemoved(addr) => {
                info!("Device removed {addr}");
            }
            _ => (),
        }
    }

    Ok(None)
}

#[tokio::main]
async fn main() -> bluer::Result<()> {
    env_logger::init();

    let mut led_device = connect_device().await.unwrap().unwrap();
    led_device.set_charasteristic().await.unwrap();
    println!("{led_device:?}");

    println!("start axum server");
    let app = Router::new()
        .route("/", get(index))
        .route("/api/set", post(set_led))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(led_device);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

// Include utf-8 file at **compile** time.
async fn index() -> Html<&'static str> {
    Html(std::include_str!("../assets/index.html"))
}

#[derive(Debug, Deserialize)]
struct SetLed {
    state: Option<String>,
    color: Option<String>,
}

async fn set_led(
    State(mut state): State<LedDevice>,
    Json(input): Json<SetLed>,
) -> impl IntoResponse {
    println!("{:?}", input);
    println!("{:?}", state);

    if let Some(on_off_state) = input.state {
        match on_off_state.as_str() {
            "on" => {
                state.turn_on().await;
            }
            "off" => {
                state.turn_off().await;
            }
            "color" => {
                if let Some(color) = input.color {
                    state.set_color(color).await;
                }
            }
            _ => {}
        }
    }
}
