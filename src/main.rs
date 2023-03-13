use bluer::{gatt::remote::Characteristic, AdapterEvent, Address, Device, Result, Uuid};
use futures::{pin_mut, StreamExt};
use std::str::FromStr;
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::sleep,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> bluer::Result<()> {
    env_logger::init();
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
    // let test = "aa010000000000000000000000000000000000ab".to_string();
    // println!("{:02x?}", test.as_bytes());

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
                                    println!("Found our characteristic!");
                                    let flags = characteristic.flags().await?;
                                    println!("Characteristic UUID: {} Flags: {:?}", &uuid,  flags);

                                    if flags.read {
                                        println!("    Reading characteristic value");
                                        let value = characteristic.read().await?;
                                        println!("    Read value: {:x?}", &value);
                                        sleep(Duration::from_secs(1)).await;
                                    }

                                    println!("    Writing characteristic value {:x?}", &off_ev);
                                    // characteristic.write(&off_ev).await?;
                                    characteristic.write(&on_ev).await?;
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
                        Ok(()) => {}
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
