// #![no_std]
// #![no_main]

// extern crate alloc;

// use esp32_nimble::{utilities, BLEDevice, NimbleProperties};
// use esp_idf_hal::delay::FreeRtos;
// use esp_idf_sys as _;
// use log::*;
// use uuid::Uuid;

// #[no_mangle]
// fn main() {
//     // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
//     // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
//     esp_idf_sys::link_patches();

//     esp_idf_svc::log::EspLogger::initialize_default();
//     // log::set_max_level(log::LevelFilter::Debug);

//     let ble_device = BLEDevice::take();

//     let server = ble_device.get_server();
//     server.on_connect(|d| {
//         info!("Client connected: {:?}", d);
//     });

//     let uuid = utilities::BleUuid::Uuid128(
//         Uuid::try_parse("fafafafa-fafa-fafa-fafa-fafafafafafa")
//             .unwrap()
//             .as_u128()
//             .to_le_bytes(),
//     );
//     let service = server.create_service(uuid);

//     let uuid = utilities::BleUuid::Uuid128(
//         Uuid::try_parse("3c9a3f00-8ed3-4bdf-8a39-a01bebede295")
//             .unwrap()
//             .as_u128()
//             .to_le_bytes(),
//     );
//     let writable_characteristic = service
//         .lock()
//         .create_characteristic(uuid, NimbleProperties::READ | NimbleProperties::WRITE);

//     writable_characteristic
//         .lock()
//         .on_read(move |v, d| {
//             ::log::info!("Read from writable characteristic: {:?} {:?}", v.value(), d);
//         })
//         .on_write(move |value, _param| {
//             ::log::info!("Wrote to writable characteristic: {:?}", value);
//         });

//     let uuid = utilities::BleUuid::Uuid128(
//         Uuid::try_parse("fafafafa-fafa-fafa-fafa-fafafafafafa")
//             .unwrap()
//             .as_u128()
//             .to_le_bytes(),
//     );

//     let ble_advertising = ble_device.get_advertising();
//     ble_advertising
//         .name("ESP32-GATT-Server-mats")
//         .add_service_uuid(uuid);

//     ble_advertising.start().unwrap();

//     loop {
//         FreeRtos::delay_ms(1000);
//     }
// }

// use anyhow::{bail, Result};
// use core::time::Duration;
// use esp_idf_hal::delay::FreeRtos;
// use esp_idf_hal::peripherals::Peripherals;
// use esp_idf_hal::rmt::config::TransmitConfig;
// use esp_idf_hal::rmt::*;

// fn main() -> Result<()> {
//     esp_idf_sys::link_patches();

//     let peripherals = Peripherals::take().unwrap();
//     // Onboard RGB LED pin
//     // ESP32-C3-DevKitC-02 gpio8, ESP32-C3-DevKit-RUST-1 gpio2
//     let led = peripherals.pins.gpio17;
//     let channel = peripherals.rmt.channel0;
//     let config = TransmitConfig::new().clock_divider(1);
//     let mut tx = TxRmtDriver::new(channel, led, &config)?;

//     // 3 seconds white at 10% brightness
//     neopixel(
//         Rgb {
//             r: 25,
//             g: 25,
//             b: 25,
//         },
//         &mut tx,
//     )?;
//     FreeRtos::delay_ms(3000);

//     // rainbow loop at 20% brightness
//     let mut i: u32 = 0;
//     loop {
//         let rgb = hsv2rgb(i, 100, 70)?;
//         neopixel(rgb, &mut tx)?;
//         if i == 360 {
//             i = 0;
//         }
//         i += 1;
//         FreeRtos::delay_ms(10);
//     }
// }

// struct Rgb {
//     r: u8,
//     g: u8,
//     b: u8,
// }

// fn ns(nanos: u64) -> Duration {
//     Duration::from_nanos(nanos)
// }

// fn neopixel(rgb: Rgb, tx: &mut TxRmtDriver) -> Result<()> {
//     // e.g. rgb: (1,2,4)
//     // G        R        B
//     // 7      0 7      0 7      0
//     // 00000010 00000001 00000100
//     let color: u32 = ((rgb.g as u32) << 16) | ((rgb.r as u32) << 8) | rgb.b as u32;
//     let ticks_hz = tx.counter_clock()?;
//     let t0h = Pulse::new_with_duration(ticks_hz, PinState::High, &ns(350))?;
//     let t0l = Pulse::new_with_duration(ticks_hz, PinState::Low, &ns(800))?;
//     let t1h = Pulse::new_with_duration(ticks_hz, PinState::High, &ns(700))?;
//     let t1l = Pulse::new_with_duration(ticks_hz, PinState::Low, &ns(600))?;
//     let mut signal = FixedLengthSignal::<24>::new();
//     for i in (0..24).rev() {
//         let p = 2_u32.pow(i);
//         let bit = p & color != 0;
//         let (high_pulse, low_pulse) = if bit { (t1h, t1l) } else { (t0h, t0l) };
//         signal.set(23 - i as usize, &(high_pulse, low_pulse))?;
//     }

//     for _ in 0..60 {
//         tx.start_blocking(&signal)?;
//     }

//     Ok(())
// }

// /// Converts hue, saturation, value to RGB
// fn hsv2rgb(h: u32, s: u32, v: u32) -> Result<Rgb> {
//     if h > 360 || s > 100 || v > 100 {
//         bail!("The given HSV values are not in valid range");
//     }
//     let s = s as f64 / 100.0;
//     let v = v as f64 / 100.0;
//     let c = s * v;
//     let x = c * (1.0 - (((h as f64 / 60.0) % 2.0) - 1.0).abs());
//     let m = v - c;
//     let (r, g, b);
//     if h < 60 {
//         r = c;
//         g = x;
//         b = 0.0;
//     } else if (60..120).contains(&h) {
//         r = x;
//         g = c;
//         b = 0.0;
//     } else if (120..180).contains(&h) {
//         r = 0.0;
//         g = c;
//         b = x;
//     } else if (180..240).contains(&h) {
//         r = 0.0;
//         g = x;
//         b = c;
//     } else if (240..300).contains(&h) {
//         r = x;
//         g = 0.0;
//         b = c;
//     } else {
//         r = c;
//         g = 0.0;
//         b = x;
//     }

//     Ok(Rgb {
//         r: ((r + m) * 255.0) as u8,
//         g: ((g + m) * 255.0) as u8,
//         b: ((b + m) * 255.0) as u8,
//     })
// }

use esp_idf_hal::delay::FreeRtos;
use esp_idf_sys::*;
use smart_leds::hsv::{hsv2rgb, Hsv, RGB};
use smart_leds_trait::SmartLedsWrite;
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

fn main() -> ! {
    // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
    // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
    esp_idf_sys::link_patches();

    let mut ws2812 = Ws2812Esp32Rmt::new(0, 17).unwrap();

    println!("Start NeoPixel rainbow!");

    let mut hue = unsafe { esp_random() } as u8;
    loop {
        let pixels = std::iter::repeat(RGB {
            r: 255,
            g: 120,
            b: 120,
        })
        .take(60);
        ws2812.write(pixels).unwrap();

        FreeRtos::delay_ms(100);

        hue = hue.wrapping_add(10);
    }
}
