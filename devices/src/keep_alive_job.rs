use bluer::gatt::remote::Characteristic;
use futures::channel::oneshot;
use log::info;
use std::{future::Future, io, pin::Pin, sync::Arc};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    sync::Mutex,
    time::{self, Duration},
};

#[derive(Debug)]
pub(crate) struct KeepAlive {
    interval: Duration,
    abort_tx: Option<oneshot::Sender<()>>,
}

impl KeepAlive {
    pub(crate) fn new(interval: Duration) -> Self {
        Self {
            interval,
            abort_tx: None,
        }
    }

    pub(crate) fn run(&mut self, characteristic: Characteristic, ev: Vec<u8>) {
        let mut interval = time::interval(self.interval);

        let (abort_tx, mut abort_rx) = oneshot::channel();
        self.abort_tx = Some(abort_tx);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        info!("Send keep alive");
                        characteristic.write(&ev).await.unwrap()
                    }
                    _ = &mut abort_rx => {
                        info!("Kill keep alive cycle");
                        return;
                    }
                }
            }
        });
    }

    pub(crate) fn stop(&mut self) {
        if let Some(notifier) = self.abort_tx.take() {
            drop(notifier);
        }
    }
}
