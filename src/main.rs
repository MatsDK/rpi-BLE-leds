use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bluer::{Address, Uuid};
use log::{debug, error, info};
use serde::Deserialize;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

use devices::{esp::EspLed, govee::GoveeLed, Devices, Event, LedDevice};

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

#[tokio::main]
async fn main() -> bluer::Result<()> {
    env_logger::init();

    let state = GlobalState::default();

    let govee_led_addr = Address::from_str("A4:C1:38:EC:91:32")?;
    let govee_service_uuid = Uuid::parse_str("000102030405060708090a0b0c0d1910").unwrap();
    let govee_characteristic_uuid = Uuid::parse_str("000102030405060708090a0b0c0d2b11").unwrap();

    let govee_leds = Devices::Govee(GoveeLed::new(
        govee_led_addr.clone(),
        govee_service_uuid,
        govee_characteristic_uuid,
    ));
    state.lock().await.add_device(govee_led_addr, govee_leds);

    let esp_led_addr = Address::from_str("40:22:D8:EA:CB:FA")?;
    let esp_service_uuid = Uuid::parse_str("1afc47f3-4a31-4c4e-9f54-ca1ede6e2e1f").unwrap();
    let esp_characteristic_uuid = Uuid::parse_str("21b3e7c8-bc41-47c7-af6c-1fe47aad759f").unwrap();

    let esp_leds = Devices::Esp(EspLed::new(
        esp_led_addr.clone(),
        esp_service_uuid,
        esp_characteristic_uuid,
    ));
    state.lock().await.add_device(esp_led_addr, esp_leds);

    let api_router = Router::new()
        .route("/set/:addr", post(set_led))
        .route("/connect/:addr", post(connect_to_led))
        .route("/disconnect/:addr", post(disconnect_from_led));

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

async fn disconnect_from_led(
    Path(addr): Path<String>,
    State(state): State<GlobalState>,
) -> impl IntoResponse {
    let addr = Address::from_str(&addr).unwrap();
    let mut state = state.lock().await;

    if let Some(device) = state.get_device(&addr) {
        match device.disconnect().await {
            Ok(()) => "Successfully disconnected".into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to disconnect: {}", e),
            )
                .into_response(),
        }
    } else {
        "Device not found".into_response()
    }
}

#[derive(Debug, Deserialize)]
pub struct SetLedEvent {
    event_type: String,
    color: Option<String>,
    brightness: Option<u8>,
    other_ev: Option<String>,
}

impl From<SetLedEvent> for Event {
    fn from(val: SetLedEvent) -> Self {
        match val.event_type.as_str() {
            "on" => Event::On,
            "off" => Event::Off,
            "color" if val.color.is_some() => Event::Color(val.color.unwrap()),
            "brightness" if val.brightness.is_some() => Event::Brightness(val.brightness.unwrap()),
            _ => Event::Other(val.other_ev),
        }
    }
}

async fn set_led(
    Path(addr): Path<String>,
    State(state): State<GlobalState>,
    Json(input): Json<SetLedEvent>,
) -> impl IntoResponse {
    let addr = Address::from_str(&addr).unwrap();
    let mut state = state.lock().await;

    if let Some(device) = state.get_device(&addr) {
        match device.on_event(input.into()).await {
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
