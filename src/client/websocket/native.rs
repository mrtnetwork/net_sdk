use std::{str::FromStr, sync::Arc};

use crate::{
    client::{IClient, IStreamClient},
    stream::ConnectStream,
    types::{config::NetConfig, error::NetResultStatus},
};
use bytes::Bytes;
use futures::{SinkExt, StreamExt, stream::SplitSink};
use http::{HeaderName, HeaderValue};
use tokio::sync::{Mutex, broadcast};
use tokio_tungstenite::{
    WebSocketStream, client_async,
    tungstenite::{Message, client::IntoClientRequest},
};
struct WriterWithHandler<T> {
    pub writer: SplitSink<WebSocketStream<Box<T>>, Message>,
}
impl<T> WriterWithHandler<T>
where
    T: ConnectStream,
{
    async fn send(&mut self, data: &[u8]) -> Result<(), NetResultStatus> {
        self.writer
            .send(Message::Binary(Bytes::copy_from_slice(data)))
            .await
            .map_err(|_| NetResultStatus::NetError)
    }
    async fn close(&mut self) {
        let _ = self.writer.send(Message::Close(None)).await;
        let _ = self.writer.close().await;
    }
}
pub struct WsStreamClient<T> {
    writer: Arc<Mutex<Option<WriterWithHandler<T>>>>,
    incoming: broadcast::Sender<Result<Option<Vec<u8>>, NetResultStatus>>,
    config: NetConfig,
}

impl<T> WsStreamClient<T>
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
impl<'a, T> IClient for WsStreamClient<T>
where
    T: ConnectStream,
{
    async fn connect(&self) -> Result<(), NetResultStatus> {
        let mut guard = self.writer.lock().await;

        if guard.is_some() {
            return Ok(()); // already connected
        }
        let stream = T::connect(&self.config).await?;
        let boxed_stream: Box<T> = Box::new(stream);
        // Inside your connect method, before calling client_async
        let mut request = self
            .config
            .addr
            .url
            .clone()
            .into_client_request()
            .map_err(|_| NetResultStatus::InvalidUrl)?;

        // Override headers from config if present
        for header in &self.config.http.headers {
            // Assume NetHttpHeader has key/value strings
            request.headers_mut().insert(
                HeaderName::from_str(&header.key)
                    .map_err(|_| NetResultStatus::InvalidRequestParameters)?,
                HeaderValue::from_str(&header.value).unwrap(),
            );
        }

        // Connect WebSocket
        let (ws_stream, _response) = client_async(request, boxed_stream)
            .await
            .map_err(|_| NetResultStatus::NetError)?;
        let (write, mut read) = ws_stream.split();

        // Spawn background reader
        let tx_clone = self.incoming.clone();
        let writer_mutex = Arc::clone(&self.writer);

        tokio::spawn(async move {
            loop {
                let msg = read.next().await;
                match msg {
                    Some(Ok(Message::Binary(data))) => {
                        let _ = tx_clone.send(Ok(Some(data.to_vec())));
                    }
                    Some(Ok(Message::Text(utf8))) => {
                        let _ = tx_clone.send(Ok(Some(utf8.as_bytes().to_vec())));
                    }
                    Some(Ok(_)) => {}
                    None => {
                        let _ = tx_clone.send(Ok(None));
                        let mut guard = writer_mutex.lock().await;
                        *guard = None;
                        break;
                    }
                    Some(Err(_)) => {
                        let _ = tx_clone.send(Err(NetResultStatus::SocketError));
                        // On disconnect, set writer to None
                        let mut guard = writer_mutex.lock().await;
                        *guard = None;
                        break;
                    }
                }
            }
        });

        // Save writer in mutex
        *guard = Some(WriterWithHandler { writer: write });

        Ok(())
    }

    fn get_config(&self) -> &NetConfig {
        return &self.config;
    }
}
#[async_trait::async_trait]
impl<T> IStreamClient for WsStreamClient<T>
where
    T: ConnectStream,
{
    async fn send<'a>(&self, data: &'a [u8]) -> Result<(), NetResultStatus> {
        self.connect().await?;
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
