#![no_std]
#![no_main]

extern crate alloc;

use esp32_nimble::{utilities, BLEDevice, NimbleProperties};
use esp_idf_hal::delay::FreeRtos;
use esp_idf_sys as _;
use log::*;
use uuid::Uuid;

#[no_mangle]
fn main() {
    // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
    // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    // log::set_max_level(log::LevelFilter::Debug);

    let ble_device = BLEDevice::take();

    let server = ble_device.get_server();
    server.on_connect(|d| {
        info!("Client connected: {:?}", d);
    });

    let uuid = utilities::BleUuid::Uuid128(
        Uuid::try_parse("fafafafa-fafa-fafa-fafa-fafafafafafa")
            .unwrap()
            .as_u128()
            .to_le_bytes(),
    );
    let service = server.create_service(uuid);

    let uuid = utilities::BleUuid::Uuid128(
        Uuid::try_parse("3c9a3f00-8ed3-4bdf-8a39-a01bebede295")
            .unwrap()
            .as_u128()
            .to_le_bytes(),
    );
    let writable_characteristic = service
        .lock()
        .create_characteristic(uuid, NimbleProperties::READ | NimbleProperties::WRITE);

    writable_characteristic
        .lock()
        .on_read(move |v, d| {
            ::log::info!("Read from writable characteristic: {:?} {:?}", v.value(), d);
        })
        .on_write(move |value, _param| {
            ::log::info!("Wrote to writable characteristic: {:?}", value);
        });

    let uuid = utilities::BleUuid::Uuid128(
        Uuid::try_parse("fafafafa-fafa-fafa-fafa-fafafafafafa")
            .unwrap()
            .as_u128()
            .to_le_bytes(),
    );

    let ble_advertising = ble_device.get_advertising();
    ble_advertising
        .name("ESP32-GATT-Server-mats")
        .add_service_uuid(uuid);

    ble_advertising.start().unwrap();

    loop {
        FreeRtos::delay_ms(1000);
    }
}
