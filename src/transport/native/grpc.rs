use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

use arti_client::DataStream;
use tokio::{
    net::TcpStream,
    sync::{
        Mutex,
        broadcast::{self},
    },
};
use tokio_rustls::client::TlsStream;

use crate::{
    client::{GrpcStreamHandle, IGrpcClient, grpc::native::GrpcClient},
    transport::{Transport, native::IGrpcTransport},
    types::{
        DartCallback,
        config::{NetConfig, NetMode, NetProtocol, NetRequestConfig},
        error::NetResultStatus,
        request::{
            NetGrpcRequestStream, NetGrpcRequestUnary, NetGrpcRequestUnsubscribe, NetRequest,
        },
        response::{
            NetGrpcResponse, NetGrpcStreamIdResponse, NetGrpcUnaryResponse,
            NetGrpcUnsubscribeStreamIdResponse, NetResponseKind, NetStreamResponse,
            NetStreamResponseData, NetStreamResponseError,
        },
    },
};

pub struct GrpcTransport {
    stream: Box<dyn IGrpcClient>,
    callback: DartCallback,
    listeners: Arc<Mutex<HashMap<i32, GrpcStreamHandle>>>,
    _transport_id: u32,
    next_stream_id: AtomicI32,
}
#[async_trait::async_trait]
impl Transport for GrpcTransport {
    fn create(
        config: NetRequestConfig,
        callback: DartCallback,
        transport_id: u32,
    ) -> Result<GrpcTransport, NetResultStatus> {
        let config: NetConfig = config.to_protocol_config(NetProtocol::Grpc)?;
        let stream: Box<dyn IGrpcClient> = match config.protocol {
            NetProtocol::Grpc => match (config.addr.is_tls, &config.mode) {
                (true, NetMode::Tor) => {
                    Box::new(GrpcClient::<TlsStream<DataStream>>::default(config)?)
                }

                (true, NetMode::Clearnet) => {
                    Box::new(GrpcClient::<TlsStream<TcpStream>>::default(config)?)
                }

                (false, NetMode::Tor) => Box::new(GrpcClient::<DataStream>::default(config)?),

                (false, NetMode::Clearnet) => Box::new(GrpcClient::<TcpStream>::default(config)?),
            },
            _ => return Err(NetResultStatus::InvalidConfigParameters),
        };
        Ok(Self {
            stream,
            callback,
            listeners: Arc::new(Mutex::new(HashMap::new())),
            _transport_id: transport_id,
            next_stream_id: AtomicI32::new(1),
        })
    }

    async fn do_request<'a>(
        &self,
        request: NetRequest<'a>,
    ) -> Result<NetResponseKind, NetResultStatus> {
        let socket_requset = request.to_grpc_request()?;
        let kind = match socket_requset {
            crate::types::request::GrpcRequest::Stream(e) => self.stream(e).await?,
            crate::types::request::GrpcRequest::Unary(e) => self.unary(e).await?,
            crate::types::request::GrpcRequest::Unsubscribe(e) => self.unsubscribe(e).await?,
        };
        Ok(kind)
    }

    async fn close(&self) {
        // Lock the mutex
        let mut listeners = self.listeners.lock().await;

        // Collect all handles into a Vec to drop the lock while closing
        let handles: Vec<GrpcStreamHandle> = listeners.drain().map(|(_, handle)| handle).collect();
        // Close each handle
        for handle in handles {
            handle.cancel();
        }
        drop(listeners);
        self.stream.close().await;
    }

    fn get_config(&self) -> &NetConfig {
        self.stream.get_config()
    }
}
#[async_trait::async_trait]
impl<'a> IGrpcTransport<'a> for GrpcTransport {
    async fn unary(
        &self,
        data: &NetGrpcRequestUnary<'a>,
    ) -> Result<NetResponseKind, NetResultStatus> {
        let r = self.stream.unary(data.data, &data.method).await?;
        Ok(NetResponseKind::Grpc(NetGrpcResponse::Unary(
            NetGrpcUnaryResponse { data: r },
        )))
    }

    async fn stream(
        &self,
        data: &NetGrpcRequestStream<'a>,
    ) -> Result<NetResponseKind, NetResultStatus> {
        let handle = self.stream.stream(data.data, &data.method).await?;
        let id = self.next_stream_id.fetch_add(1, Ordering::Relaxed);
        let callback = self.callback.clone();
        let mut rx = handle.rx.resubscribe();
        let listeners = Arc::clone(&self.listeners);
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(msg) => match msg {
                        Ok(data) => match data {
                            Some(b) => {
                                callback(NetResponseKind::Stream(NetStreamResponse::Data(
                                    NetStreamResponseData {
                                        data: b,
                                        id: Some(id),
                                    },
                                )));
                            }
                            None => {
                                callback(NetResponseKind::Stream(NetStreamResponse::Close(Some(
                                    id,
                                ))));
                                break;
                            }
                        },
                        Err(err) => {
                            callback(NetResponseKind::Stream(NetStreamResponse::Error(
                                NetStreamResponseError {
                                    id: Some(id),
                                    status: err,
                                },
                            )));
                            break;
                        }
                    },
                    Err(broadcast::error::RecvError::Closed) => {
                        callback(NetResponseKind::Stream(NetStreamResponse::Close(Some(id))));
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // optional: report backpressure
                    }
                }
            }
            let mut guard = listeners.lock().await;
            guard.remove(&id);
        });
        // 3. Store it
        {
            let mut listeners = self.listeners.lock().await;
            listeners.insert(id, handle);
        }

        // 4. Return ID to caller
        Ok(NetResponseKind::Grpc(NetGrpcResponse::StreamId(
            NetGrpcStreamIdResponse { id },
        )))
    }
    async fn unsubscribe(
        &self,
        data: &NetGrpcRequestUnsubscribe,
    ) -> Result<NetResponseKind, NetResultStatus> {
        let mut listeners = self.listeners.lock().await;
        let handler = listeners.remove(&data.id);
        let _ = match handler {
            Some(e) => e.cancel(),
            None => (),
        };
        // 4. Return ID to caller
        Ok(NetResponseKind::Grpc(NetGrpcResponse::Unsubscribe(
            NetGrpcUnsubscribeStreamIdResponse { id: data.id },
        )))
    }
}
