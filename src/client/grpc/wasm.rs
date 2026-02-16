use crate::client::wasm::{GrpcStreamHandle, IClient, IGrpcClient};
use crate::{
    client::grpc::raw_codec::BufferCodec,
    types::{config::NetConfig, error::NetResultStatus},
};
use futures::stream;
use http::uri::PathAndQuery;
use std::{marker::PhantomData, sync::Arc};
use tokio::sync::{Mutex, broadcast, oneshot};
use tonic::{Code, client::Grpc};
use tonic_web_wasm_client::Client;
use wasm_bindgen_futures::spawn_local;
pub struct GrpcClient {
    client: Arc<Mutex<Option<Grpc<Client>>>>,
    config: NetConfig,
    _marker: PhantomData<()>,
}

impl GrpcClient {
    pub fn default(config: NetConfig) -> Result<Self, NetResultStatus> {
        Ok(Self {
            client: Arc::new(Mutex::new(None)),
            config,
            _marker: PhantomData,
        })
    }
}

#[async_trait::async_trait(?Send)]
impl IClient for GrpcClient {
    async fn connect(&self) -> Result<(), NetResultStatus> {
        let mut guard = self.client.lock().await;
        let reconnect_needed = match guard.as_mut() {
            Some(client) => client.ready().await.is_err(),
            None => true,
        };
        if reconnect_needed {
            let wasm_client = Client::new(self.config.addr.url.clone());
            let grpc = Grpc::new(wasm_client);
            *guard = Some(grpc);
        }

        Ok(())
    }

    fn get_config(&self) -> &NetConfig {
        &self.config
    }
}

#[async_trait::async_trait(?Send)]
impl IGrpcClient for GrpcClient {
    async fn unary(&self, buffer: &[u8], method_name: &str) -> Result<Vec<u8>, NetResultStatus> {
        self.connect().await?;
        let mut guard = self.client.lock().await;
        let client = guard.as_mut().ok_or(NetResultStatus::ConnectionError)?; // should exist after connect()

        let path = PathAndQuery::try_from(method_name.to_string())
            .map_err(|_| NetResultStatus::InvalidRequestParameters)?;
        let req = tonic::Request::new(Vec::from(buffer));
        let codec = BufferCodec::default();
        let resp = client
            .unary(req, path, codec)
            .await
            .map_err(|_| NetResultStatus::ConnectionError)?;
        Ok(resp.into_inner())
    }

    async fn stream(
        &self,
        buffer: &[u8],
        method_name: &str,
    ) -> Result<GrpcStreamHandle, NetResultStatus> {
        self.connect().await?;
        let (tx, rx) = broadcast::channel(128);
        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
        // Lock mutex to get client
        let mut guard = self.client.lock().await;
        let client = guard.as_mut().ok_or(NetResultStatus::ConnectionError)?; // should exist after connect()

        let path = PathAndQuery::try_from(method_name.to_string())
            .map_err(|_| NetResultStatus::InvalidRequestParameters)?;
        let codec = BufferCodec::default();
        let buffer = Vec::from(buffer);
        let req_stream = stream::once(async { buffer });
        let req = tonic::Request::new(req_stream);

        client
            .ready()
            .await
            .map_err(|_| NetResultStatus::ConnectionError)?;
        let stream = client
            .streaming(req, path, codec)
            .await
            .map_err(|_| NetResultStatus::ConnectionError)?;
        let mut stream: tonic::Streaming<Vec<u8>> = stream.into_inner();

        let tx_clone: broadcast::Sender<Result<Option<Vec<u8>>, NetResultStatus>> = tx.clone();
        spawn_local(async move {
            loop {
                tokio::select! {
                    _ = &mut cancel_rx => {
                        // cancel requested
                        break;
                    }
                    msg = stream.message() => {
                        match msg {
                            Ok(Some(msg)) => {
                                let _ = tx_clone.send(Ok(Some(msg.to_vec())));
                            },
                            Ok(None) => {
                                let _ = tx_clone.send(Ok(None));
                                break;
                            }
                            Err(err) => {
                                     if err.code()==Code::Ok{
                                           let _ = tx_clone.send(Ok(None));
                                     }else{
                                         let _ = tx_clone.send(Err(NetResultStatus::SocketError));
                                     }
                                    break;

                            }

                        }
                    }
                }
            }
        });

        Ok(GrpcStreamHandle {
            rx,
            cancel: cancel_tx,
        })
    }

    async fn close(&self) {
        let mut guard = self.client.lock().await;
        *guard = None;
    }
}
