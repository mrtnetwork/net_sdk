use std::sync::Arc;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex, broadcast},
};

use crate::{
    client::{IClient, IStreamClient},
    stream::ConnectStream,
    types::{config::NetConfig, error::NetResultStatus},
};

struct WriterWithHandler<T> {
    pub writer: tokio::io::WriteHalf<T>,
}
impl<T> WriterWithHandler<T>
where
    T: ConnectStream,
{
    async fn send(&mut self, data: &[u8]) -> Result<(), NetResultStatus> {
        let _ = self
            .writer
            .write_all(&data)
            .await
            .map_err(|_| NetResultStatus::NetError);
        self.writer
            .flush()
            .await
            .map_err(|_| NetResultStatus::NetError)
    }
    async fn close(&mut self) {
        // let _ = self.send(&[0xff]).await;
        let _ = self.writer.shutdown().await;
    }
}

pub struct RawStreamClient<T> {
    incoming: broadcast::Sender<Result<Option<Vec<u8>>, NetResultStatus>>,
    writer: Arc<Mutex<Option<WriterWithHandler<T>>>>,
    config: NetConfig,
}

impl<'a, T> RawStreamClient<T>
where
    T: ConnectStream,
{
    pub fn default(config: NetConfig) -> Result<Self, NetResultStatus> {
        let (tx, _) = broadcast::channel(128);
        Ok(Self {
            incoming: tx,
            writer: Arc::new(Mutex::new(None)),
            config: config,
        })
    }
}
#[async_trait::async_trait]
impl<'a, T> IClient for RawStreamClient<T>
where
    T: ConnectStream,
{
    async fn connect(&self) -> Result<(), NetResultStatus> {
        let mut guard = self.writer.lock().await;

        // Already connected
        if guard.is_some() {
            return Ok(());
        }
        // Create new connection
        let stream = T::connect(&self.config).await?;
        let (mut reader, writer) = tokio::io::split(stream);

        // Clone for background task
        let writer_mutex = Arc::clone(&self.writer);
        let tx_clone = self.incoming.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => {
                        let _ = tx_clone.send(Ok(None));
                        break;
                    } // EOF
                    Ok(n) => {
                        let _ = tx_clone.send(Ok(Some(buf[..n].to_vec())));
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::ConnectionReset
                            || e.kind() == std::io::ErrorKind::BrokenPipe =>
                    {
                        let _ = tx_clone.send(Ok(None));
                        break;
                    }
                    Err(_) => {
                        let _ = tx_clone.send(Err(NetResultStatus::SocketError));
                        break;
                    }
                }
            }
            // Connection closed → set writer to None
            let mut guard = writer_mutex.lock().await;
            *guard = None;
        });
        *guard = Some(WriterWithHandler { writer });

        Ok(())
    }
    fn get_config(&self) -> &NetConfig {
        return &self.config;
    }
}

#[async_trait::async_trait]
impl<T> IStreamClient for RawStreamClient<T>
where
    T: ConnectStream,
{
    async fn send<'a>(&self, data: &'a [u8]) -> Result<(), NetResultStatus> {
        self.connect().await?; // ensure connection exists

        let mut guard = self.writer.lock().await;

        if let Some(writer) = guard.as_mut() {
            writer.send(&data).await
        } else {
            Err(NetResultStatus::NetError)
        }
    }

    async fn subscribe(
        &self,
    ) -> Result<broadcast::Receiver<Result<Option<Vec<u8>>, NetResultStatus>>, NetResultStatus>
    {
        self.connect().await?;
        Ok(self.incoming.subscribe())
    }
    async fn close(&self) {
        let mut guard = self.writer.lock().await;
        if let Some(writer) = guard.as_mut() {
            let _ = writer.close().await;
        }
        *guard = None
    }
}
