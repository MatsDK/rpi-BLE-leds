#![no_std]
#![no_main]

extern crate alloc;

use esp32_nimble::BLEDevice;
use esp_idf_hal::task::executor::{EspExecutor, Local};
use esp_idf_sys as _;
use log::*;

#[no_mangle]
fn main() {
    // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
    // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
    esp_idf_sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(log::LevelFilter::Debug);

    let executor = EspExecutor::<16, Local>::new();
    let _task = executor
        .spawn_local(async {
            let ble_device = BLEDevice::take();
            let ble_scan = ble_device.get_scan();
            ble_scan
                .active_scan(true)
                .interval(100)
                .window(99)
                .on_result(|param| {
                    info!("Advertised Device: {:?}", param);
                });
            ble_scan.start(10000).await.unwrap();
            info!("Scan end");
        })
        .unwrap();

    executor.run(|| true);
}


