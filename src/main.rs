use bluer::{gatt::remote::Characteristic, AdapterEvent, Address, Device, Result, Uuid};
use futures::{pin_mut, StreamExt};
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
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

async fn connect_device() -> bluer::Result<()> {
    let turn_on = env::args().any(|arg| arg == "--on");
    let turn_off = env::args().any(|arg| arg == "--off");

    if !turn_on && !turn_off {
        return Ok(());
    }

    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;

    let led_addr = Address::from_str("A4:C1:38:EC:91:32")?;
    let led_service_uuid = Uuid::parse_str("000102030405060708090a0b0c0d1910").unwrap();
    let led_char_uuid = Uuid::parse_str("000102030405060708090a0b0c0d2b11").unwrap();

    let on_ev = vec![
        0x33, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x33,
    ];
    let off_ev = vec![
        0x33, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x32,
    ];

    println!(
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
                if addr == led_addr {
                    let uuids = device.uuids().await?.unwrap_or_default();

                    println!("Discovered device {} with service UUIDs {:?}", addr, &uuids);

                    if !device.is_connected().await? {
                        println!("    Connecting...");
                        let mut retries = 2;
                        loop {
                            match device.connect().await {
                                Ok(()) => break,
                                Err(err) if retries > 0 => {
                                    println!("    Connect error: {}", &err);
                                    retries -= 1;
                                }
                                Err(err) => return Err(err),
                            }
                        }
                        println!("    Connected");
                    } else {
                        println!("    Already connected");
                    }

                    for service in device.services().await? {
                        let uuid = service.uuid().await?;
                        if led_service_uuid == uuid {
                            println!("Found service with uuid: {:?}", uuid);
                            for characteristic in service.characteristics().await? {
                                let uuid = characteristic.uuid().await?;

                                if uuid == led_char_uuid {
                                    println!("Found our characteristic {}", uuid);
                                    let flags = characteristic.flags().await?;
                                    // println!("Characteristic UUID: {} Flags: {:?}", &uuid, flags);

                                    // if flags.read {
                                    //     println!("    Reading characteristic value");
                                    //     let value = characteristic.read().await?;
                                    //     println!("    Read value: {:x?}", &value);
                                    //     sleep(Duration::from_secs(1)).await;
                                    // }

                                    println!("{turn_on} {turn_off}");
                                    if turn_on {
                                        characteristic.write(&on_ev).await?;
                                    }
                                    if turn_off {
                                        characteristic.write(&off_ev).await?;
                                    }
                                }
                                // println!(
                                //     "    Characteristic data: {:?}",
                                //     characteristic.all_properties().await?
                                // );
                            }
                        }
                    }

                    println!("Disconnecting");
                    match device.disconnect().await {
                        Ok(()) => break,
                        Err(err) => {}
                    }
                }
            }
            AdapterEvent::DeviceRemoved(addr) => {
                println!("Device removed {addr}");
            }
            _ => (),
        }
    }
    println!("Stopping discovery");

    sleep(Duration::from_secs(1)).await;

    Ok(())
}

// #[tokio::main(flavor = "current_thread")]
#[tokio::main]
async fn main() -> bluer::Result<()> {
    let _ = connect_device().await;

    // build our application with a single route
    println!("start axum server");
    let app = Router::new()
        .route("/", get(index))
        .route("/api/set", post(set_led))
        .nest_service("/assets", ServeDir::new("assets"));

    // run it with hyper on localhost:3000
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

async fn set_led(Json(input): Json<SetLed>) -> impl IntoResponse {
    println!("{:?}", input);
}
