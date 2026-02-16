use std::{str::FromStr, sync::Arc};

use crate::client::wasm::{IClient, IStreamClient};
use crate::types::{config::NetConfig, error::NetResultStatus};
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use http::{HeaderName, HeaderValue};
use tokio::sync::{Mutex, broadcast};
use wasm_bindgen_futures::spawn_local;
use ws_stream_wasm::{WsMessage, WsMeta};

struct WriterWithHandler {
    writer: futures::stream::SplitSink<ws_stream_wasm::WsStream, WsMessage>,
}

impl WriterWithHandler {
    async fn send(&mut self, data: &[u8]) -> Result<(), NetResultStatus> {
        self.writer
            .send(WsMessage::Binary(Bytes::copy_from_slice(data).to_vec()))
            .await
            .map_err(|_| NetResultStatus::ConnectionError)
    }

    async fn close(&mut self) {
        // self.writer.poll_close_unpin(cx);
        let _ = self.writer.close().await;
    }
}

pub struct WsStreamClient {
    writer: Arc<Mutex<Option<WriterWithHandler>>>,
    incoming: broadcast::Sender<Result<Option<Vec<u8>>, NetResultStatus>>,
    config: NetConfig,
}

impl WsStreamClient {
    pub fn default(config: NetConfig) -> Result<Self, NetResultStatus> {
        let (tx, _) = broadcast::channel(128);

        Ok(Self {
            incoming: tx,
            writer: Arc::new(Mutex::new(None)),
            config,
        })
    }
}

#[async_trait::async_trait(?Send)]
impl IClient for WsStreamClient {
    async fn connect(&self) -> Result<(), NetResultStatus> {
        let mut guard = self.writer.lock().await;

        if guard.is_some() {
            return Ok(());
        }

        let url = self.config.addr.url.clone();

        // Headers (browser only allows limited custom headers!)
        let mut request = http::Request::builder()
            .uri(&url)
            .body(())
            .map_err(|_| NetResultStatus::InvalidUrl)?;

        for header in &self.config.http.headers {
            request.headers_mut().insert(
                HeaderName::from_str(&header.key())
                    .map_err(|_| NetResultStatus::InvalidRequestParameters)?,
                HeaderValue::from_str(&header.value())
                    .map_err(|_| NetResultStatus::InvalidRequestParameters)?,
            );
        }

        let (_, ws_stream) = WsMeta::connect(url, None)
            .await
            .map_err(|_| NetResultStatus::ConnectionError)?;

        let (write, mut read) = ws_stream.split();

        let tx_clone = self.incoming.clone();
        let writer_mutex = Arc::clone(&self.writer);
        // ws_stream.
        spawn_local(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    WsMessage::Binary(data) => {
                        let _ = tx_clone.send(Ok(Some(data)));
                    }
                    WsMessage::Text(text) => {
                        let _ = tx_clone.send(Ok(Some(text.into_bytes())));
                    }
                }
            }
            let _ = tx_clone.send(Ok(None));

            let mut guard = writer_mutex.lock().await;
            *guard = None;
        });

        *guard = Some(WriterWithHandler { writer: write });

        Ok(())
    }

    fn get_config(&self) -> &NetConfig {
        &self.config
    }
}

#[async_trait::async_trait(?Send)]
impl IStreamClient for WsStreamClient {
    async fn send(&self, data: &[u8]) -> Result<(), NetResultStatus> {
        self.connect().await?;

        let mut guard = self.writer.lock().await;

        if let Some(writer) = guard.as_mut() {
            writer.send(data).await
        } else {
            Err(NetResultStatus::ConnectionError)
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
            writer.close().await;
        }

        *guard = None;
    }
}
