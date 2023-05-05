use bluer::gatt::remote::Characteristic;
use log::info;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    sync::Mutex,
    time::{self, Duration},
};

#[derive(Debug)]
pub(crate) struct KeepAlive {
    interval: Duration,
}

impl KeepAlive {
    pub(crate) fn new(interval: Duration) -> Self {
        Self { interval }
    }

    pub(crate) fn run(&self, characteristic: Characteristic, ev: Vec<u8>) {
        let mut interval = time::interval(self.interval);

        tokio::spawn(async move {
            loop {
                interval.tick().await;
                info!("Send keep alive");
                characteristic.write(&ev).await.unwrap()
            }
        });
    }
}
