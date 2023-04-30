use log::info;
use std::sync::Arc;
use std::{future::Future, pin::Pin};
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

#[derive(Debug)]
pub(crate) struct KeepAlive {
    interval: Duration,
}

impl KeepAlive {
    pub(crate) fn new(interval: Duration) -> Self {
        Self { interval }
    }

    pub(crate) fn run<W, Fut>(
        &self,
        mut w: Arc<Mutex<W>>,
        cb: impl Fn() -> Fut + Send + Sync + 'static,
    ) where
        Fut: Future<Output = Result<Vec<u8>, ()>> + Send + Sync,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let mut interval = time::interval(self.interval);

        tokio::spawn(async move {
            loop {
                interval.tick().await;
                match cb().await {
                    Ok(ev) => {
                        info!("Write keep alive");
                        let mut writer = w.lock().await;
                        writer.write(&ev).await.unwrap();
                    }
                    Err(_) => {}
                };
            }
        });
    }
}
