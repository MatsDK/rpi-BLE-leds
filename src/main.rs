use askama::Template;
use bluer::{gatt::remote::Characteristic, AdapterEvent, Address, Device, Uuid};
use futures::{pin_mut, StreamExt};
use log::{debug, error, info};
use std::{
    collections::HashMap,
    io::{self, Error, ErrorKind},
    str::FromStr,
    sync::Arc,
};

use tokio::sync::Mutex;

use async_trait::async_trait;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use tower_http::services::ServeDir;

#[async_trait]
trait LedDevice {
    async fn connect(&mut self) -> io::Result<()>;

    async fn on_event(&mut self, event: SetLedEvent) -> io::Result<()>;
}

#[derive(Debug, Clone)]
struct GoveeLed {
    addr: Address,
    service_uuid: Uuid,
    characteristic_uuid: Uuid,
    device: Option<Device>,
    characteristic: Option<Characteristic>,
}

impl GoveeLed {
    fn new(addr: Address, service_uuid: Uuid, characteristic_uuid: Uuid) -> Self {
        Self {
            addr,
            service_uuid,
            characteristic_uuid,
            device: None,
            characteristic: None,
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
                    self.characteristic = Some(characteristic);
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

    async fn on_event(&mut self, event: SetLedEvent) -> io::Result<()> {
        info!("Set led on {:?}, {:?}", self.addr, event);

        match event.event_type.as_str() {
            "on" => self.turn_on().await,
            "off" => self.turn_off().await,
            "color" if event.color.is_some() => self.set_color(event.color.unwrap()).await,
            _ => {}
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Other {}

impl Other {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl LedDevice for Other {
    async fn connect(&mut self) -> io::Result<()> {
        Ok(())
    }

    async fn on_event(&mut self, event: SetLedEvent) -> io::Result<()> {
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

#[derive(Debug, Clone)]
struct DevicesState<T: LedDevice> {
    devices: HashMap<Address, T>,
}

impl<T: LedDevice> DevicesState<T> {
    fn add_device(&mut self, addr: Address, device: T) {
        self.devices.insert(addr, device);
    }

    fn get_device(&mut self, addr: &Address) -> Option<&mut T> {
        self.devices.get_mut(addr)
    }

    fn get_device_names(&self) -> Vec<String> {
        self.devices.keys().map(|a| a.to_string()).collect()
    }
}

impl<T> Default for DevicesState<T>
where
    T: LedDevice,
{
    fn default() -> DevicesState<T> {
        Self {
            devices: Default::default(),
        }
    }
}

type GlobalState = Arc<Mutex<DevicesState<Devices>>>;

#[derive(Debug, Clone)]
enum Devices {
    Govee(GoveeLed),
    Other(Other),
}

// TODO: add macro to extract this
#[async_trait]
impl LedDevice for Devices {
    async fn connect(&mut self) -> io::Result<()> {
        match self {
            Devices::Govee(d) => d.connect().await,
            Devices::Other(d) => d.connect().await,
        }
    }

    async fn on_event(&mut self, event: SetLedEvent) -> io::Result<()> {
        match self {
            Devices::Govee(d) => d.on_event(event).await,
            Devices::Other(d) => d.on_event(event).await,
        }
    }
}

#[tokio::main]
async fn main() -> bluer::Result<()> {
    env_logger::init();

    let led_addr = Address::from_str("A4:C1:38:EC:91:32")?;
    let service_uuid = Uuid::parse_str("000102030405060708090a0b0c0d1910").unwrap();
    let characteristic_uuid = Uuid::parse_str("000102030405060708090a0b0c0d2b11").unwrap();

    let govee_leds = Devices::Govee(GoveeLed::new(
        led_addr.clone(),
        service_uuid,
        characteristic_uuid,
    ));

    let _other = Devices::Other(Other::new());

    let state = GlobalState::default();
    state.lock().await.add_device(led_addr, govee_leds);

    let api_router = Router::new()
        .route("/set/:addr", post(set_led))
        .route("/connect/:addr", post(connect_to_led));

    let app_router = Router::new()
        .route("/", get(index))
        .nest("/api", api_router)
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state);

    println!("start axum server");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app_router.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn connect_to_led(
    Path(addr): Path<String>,
    State(state): State<GlobalState>,
) -> impl IntoResponse {
    let addr = Address::from_str(&addr).unwrap();
    let mut state = state.lock().await;

    if let Some(device) = state.get_device(&addr) {
        match device.connect().await {
            Ok(()) => "Successfully connected".into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to connect: {}", e),
            )
                .into_response(),
        }
    } else {
        "Device not found".into_response()
    }
}

#[derive(Debug, Deserialize)]
struct SetLedEvent {
    event_type: String,
    color: Option<String>,
}

async fn set_led(
    Path(addr): Path<String>,
    State(state): State<GlobalState>,
    Json(input): Json<SetLedEvent>,
) -> impl IntoResponse {
    let addr = Address::from_str(&addr).unwrap();
    let mut state = state.lock().await;

    if let Some(device) = state.get_device(&addr) {
        match device.on_event(input).await {
            _ => {}
        }
    }
}

async fn index(State(state): State<GlobalState>) -> impl IntoResponse {
    let state = state.lock().await;

    let template = IndexTemplate {
        devices: serde_json::to_string(&state.get_device_names()).unwrap(),
    };
    HtmlTemplate(template)
}

#[derive(Template)]
#[template(path = "index.html", escape = "none")]
struct IndexTemplate {
    devices: String,
}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", err),
            )
                .into_response(),
        }
    }
}
