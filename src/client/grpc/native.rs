use futures::stream;
use log::debug;
use std::{marker::PhantomData, sync::Arc};
use tokio::sync::{Mutex, broadcast, oneshot};
use tokio_tungstenite::tungstenite::http::uri::PathAndQuery;
use tonic::{Code, client::Grpc, transport::Channel};

use crate::{
    client::{
        grpc::raw_codec::BufferCodec,
        native::{GrpcStreamHandle, IClient, IGrpcClient},
    },
    stream::{ConnectStream, grpc::GrpcConnector},
    types::{config::NetConfig, error::NetResultStatus},
};

pub struct GrpcClient<T> {
    client: Arc<Mutex<Option<Grpc<Channel>>>>,
    config: NetConfig,
    _marker: PhantomData<T>,
}
impl<T> GrpcClient<T>
where
    T: ConnectStream,
{
    pub fn default(config: NetConfig) -> Result<Self, NetResultStatus> {
        Ok(Self {
            client: Arc::new(Mutex::new(None)),
            config: config,
            _marker: PhantomData,
        })
    }
}
#[async_trait::async_trait]
impl<T> IClient for GrpcClient<T>
where
    T: ConnectStream,
{
    async fn connect(&self) -> Result<(), NetResultStatus> {
        let mut guard = self.client.lock().await;

        let reconnect_needed = match guard.as_mut() {
            Some(client) => client.ready().await.is_err(),
            None => true,
        };
        if reconnect_needed {
            // Create new channel
            let endpoint = tonic::transport::Endpoint::from_shared(self.config.addr.url.clone())
                .map_err(|e| {
                    debug!(
                        "Create Grpc channel error: {:#?}, {:#?} ",
                        e, self.config.addr.url
                    );
                    NetResultStatus::InvalidRequestParameters
                })?;
            let connector = GrpcConnector::<T>::default(&self.config);
            let channel = endpoint
                .connect_with_connector(connector)
                .await
                .map_err(|e| {
                    debug!("Grpc client error: {:#?}, {:#?} ", e, self.config.addr.url);
                    NetResultStatus::ConnectionError
                })?;
            *guard = Some(Grpc::new(channel));
        }

        Ok(())
    }

    fn get_config(&self) -> &NetConfig {
        return &self.config;
    }
}
#[async_trait::async_trait]
impl<T> IGrpcClient for GrpcClient<T>
where
    T: ConnectStream,
{
    async fn unary<'a>(
        &self,
        buffer: &'a [u8],
        method_name: &'a str,
    ) -> Result<Vec<u8>, NetResultStatus> {
        self.connect().await?;
        let mut guard = self.client.lock().await;
        let client = guard.as_mut().ok_or(NetResultStatus::InternalError)?; // should exist after connect()
        let path = PathAndQuery::try_from(method_name.to_string()).map_err(|e| {
            debug!("Config grpc query path error: {:#?}", e);
            NetResultStatus::InvalidRequestParameters
        })?;
        let req = tonic::Request::new(Vec::from(buffer));
        let codec = BufferCodec::default();

        client.ready().await.map_err(|e| {
            debug!("Grpc client error: {:#?}", e);
            NetResultStatus::ConnectionError
        })?;
        let resp = client.unary(req, path, codec).await.map_err(|e| {
            debug!("Grpc unary requeset error: {:#?}", e);
            NetResultStatus::ConnectionError
        })?;
        Ok(resp.into_inner())
    }

    async fn stream<'a>(
        &self,
        buffer: &'a [u8],
        method_name: &'a str,
    ) -> Result<GrpcStreamHandle, NetResultStatus> {
        self.connect().await?;
        let (tx, rx) = broadcast::channel(128);
        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
        // Lock mutex to get client
        let mut guard = self.client.lock().await;
        let client = guard.as_mut().ok_or(NetResultStatus::ConnectionError)?; // should exist after connect()

        let path = PathAndQuery::try_from(method_name.to_string()).map_err(|e| {
            debug!("Grpc stream config query path error: {:#?}", e);
            NetResultStatus::InvalidRequestParameters
        })?;
        let codec = BufferCodec::default();
        let buffer = Vec::from(buffer);
        let req_stream = stream::once(async { buffer });
        let req = tonic::Request::new(req_stream);

        client.ready().await.map_err(|e| {
            debug!("Grpc client error: {:#?}", e);
            NetResultStatus::ConnectionError
        })?;
        let stream = client.streaming(req, path, codec).await.map_err(|e| {
            debug!("Grpc streaming request error: {:#?}", e);
            NetResultStatus::ConnectionError
        })?;
        let mut stream: tonic::Streaming<Vec<u8>> = stream.into_inner();

        let tx_clone: broadcast::Sender<Result<Option<Vec<u8>>, NetResultStatus>> = tx.clone();
        tokio::spawn(async move {
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
                                    debug!("Grpc streaming on message error: {:#?}", err);
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
        let mut client = self.client.lock().await;
        *client = None;
        debug!("Grpc client close.");
    }
}
