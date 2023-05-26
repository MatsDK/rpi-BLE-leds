#![no_std]
#![no_main]

extern crate alloc;

use esp32_nimble::{utilities::BleUuid, BLEDevice, NimbleProperties};
use esp_idf_hal::delay::FreeRtos;
use esp_idf_sys as _;
use log::*;
use smart_leds::hsv::RGB;
use smart_leds_trait::SmartLedsWrite;
use uuid::Uuid;
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

const LED_PIN: u32 = 17;
const NUM_LEDS: usize = 60;

#[no_mangle]
fn main() {
    // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
    // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    // log::set_max_level(log::LevelFilter::Debug);

    let mut ws2812 = Ws2812Esp32Rmt::new(0, LED_PIN).unwrap();

    // Turn all leds off on init
    let pixels = get_pixel_colors(Event::State(false));
    ws2812.write(pixels.into_iter()).unwrap();

    let ble_device = BLEDevice::take();

    let server = ble_device.get_server();
    server.on_connect(|d| {
        info!("Client connected: {:?}", d);
    });

    let uuid = str_to_uuid("1afc47f3-4a31-4c4e-9f54-ca1ede6e2e1f");
    let service = server.create_service(uuid);

    let uuid = str_to_uuid("21b3e7c8-bc41-47c7-af6c-1fe47aad759f");
    let writable_characteristic = service
        .lock()
        .create_characteristic(uuid, NimbleProperties::READ | NimbleProperties::WRITE);

    writable_characteristic
        .lock()
        .on_read(move |v, d| {
            ::log::info!("Read from writable characteristic: {:?} {:?}", v.value(), d);
        })
        .on_write(move |value, _param| {
            let pixels = get_pixel_colors(value.into());
            ws2812.write(pixels.into_iter()).unwrap();

            ::log::info!("Wrote to writable characteristic: {:?}", value);
        });


    let uuid = str_to_uuid("1afc47f3-4a31-4c4e-9f54-ca1ede6e2e1f");
    let mut ble_advertising = ble_device.get_advertising();
    ble_advertising =  ble_advertising
        .name("ESP32-GATT-Server-mats")
        .add_service_uuid(uuid);

    ble_advertising.start().unwrap();

    loop {
        FreeRtos::delay_ms(1000);
    }
}

fn str_to_uuid(s: &str) -> BleUuid {
    BleUuid::Uuid128(Uuid::try_parse(s).unwrap().as_u128().to_le_bytes())
}

const INIT: RGB<u8> = RGB { r: 0, g: 0, b: 0 };

fn get_pixel_colors(e: Event) -> [RGB<u8>; NUM_LEDS] {
    let mut pixels = [INIT; NUM_LEDS];

    match e {
        Event::State(s) => {
            for i in 0..NUM_LEDS {
                pixels[i] = if !s {
                    RGB { r: 0, g: 0, b: 0 }
                } else {
                    RGB { r: 0, g: 0, b: 0 }
                }
            }
        }
        Event::Brightness(_) => {}
        Event::Color((r, g, b)) => {
            for i in 0..NUM_LEDS {
                pixels[i] = RGB { r, g, b }
            }
        }
        Event::Other(_) => {}
    }

    pixels
}

enum Event {
    Color((u8, u8, u8)),
    Brightness(u8),
    State(bool),
    Other(u8),
}

impl From<&[u8]> for Event {
    fn from(value: &[u8]) -> Self {
        match value[0] {
            0x01 => {
                let s = value[1] == 1;
                Event::State(s)
            }
            0x02 => Event::Brightness(value[1]),
            0x03 => Event::Color((value[1], value[2], value[3])),
            _ => Event::Other(value[1]),
        }
    }
}
