use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

use tokio::sync::{Mutex, broadcast};
use wasm_bindgen_futures::spawn_local;

use crate::{
    client::{
        grpc::wasm::GrpcClient,
        wasm::{GrpcStreamHandle, IGrpcClient},
    },
    transport::wasm::{IGrpcTransport, Transport},
    types::{
        DartCallback,
        config::{NetConfig, NetConfigRequest, NetProtocol},
        error::NetResultStatus,
        request::{
            NetRequest, NetRequestGrpcStream, NetRequestGrpcUnary, NetRequestGrpcUnsubscribe,
        },
        response::{
            NetResponseGrpc, NetResponseGrpcSubscribe, NetResponseGrpcUnary,
            NetResponseGrpcUnsubscribe, NetResponseKind, NetResponseStream, NetResponseStreamData,
            NetResponseStreamError,
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
#[async_trait::async_trait(?Send)]
impl Transport for GrpcTransport {
    fn create(
        config: NetConfigRequest,
        callback: DartCallback,
        transport_id: u32,
    ) -> Result<GrpcTransport, NetResultStatus> {
        let config: NetConfig = config.to_protocol_config(NetProtocol::Grpc)?;
        let client = GrpcClient::default(config)?;
        Ok(Self {
            stream: Box::new(client),
            callback,
            listeners: Arc::new(Mutex::new(HashMap::new())),
            _transport_id: transport_id,
            next_stream_id: AtomicI32::new(1),
        })
    }

    async fn do_request(&self, request: NetRequest) -> Result<NetResponseKind, NetResultStatus> {
        let socket_requset = request.to_grpc_request()?;
        let kind = match socket_requset {
            crate::types::request::NetRequestGrpc::Stream(e) => self.stream(e).await?,
            crate::types::request::NetRequestGrpc::Unary(e) => self.unary(e).await?,
            crate::types::request::NetRequestGrpc::Unsubscribe(e) => self.unsubscribe(e).await?,
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
#[async_trait::async_trait(?Send)]
impl IGrpcTransport for GrpcTransport {
    async fn unary(&self, data: &NetRequestGrpcUnary) -> Result<NetResponseKind, NetResultStatus> {
        let data = self.stream.unary(data.data(), data.method()).await?;
        Ok(NetResponseKind::Grpc(NetResponseGrpc::Unary(
            NetResponseGrpcUnary::new(data),
        )))
    }

    async fn stream(
        &self,
        data: &NetRequestGrpcStream,
    ) -> Result<NetResponseKind, NetResultStatus> {
        let handle = self.stream.stream(data.data(), data.method()).await?;
        let id = self.next_stream_id.fetch_add(1, Ordering::Relaxed);
        let callback = self.callback.clone();
        let mut rx = handle.rx.resubscribe();
        let listeners = Arc::clone(&self.listeners);
        spawn_local(async move {
            loop {
                match rx.recv().await {
                    Ok(msg) => match msg {
                        Ok(data) => match data {
                            Some(b) => {
                                callback(NetResponseKind::Stream(NetResponseStream::Data(
                                    NetResponseStreamData::new(Some(id), b),
                                )));
                            }
                            None => {
                                callback(NetResponseKind::Stream(NetResponseStream::Close(Some(
                                    id,
                                ))));
                                break;
                            }
                        },
                        Err(err) => {
                            callback(NetResponseKind::Stream(NetResponseStream::Error(
                                NetResponseStreamError::new(Some(id), err),
                            )));
                            break;
                        }
                    },
                    Err(broadcast::error::RecvError::Closed) => {
                        callback(NetResponseKind::Stream(NetResponseStream::Close(Some(id))));
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
        Ok(NetResponseKind::Grpc(NetResponseGrpc::StreamId(
            NetResponseGrpcSubscribe::new(id),
        )))
    }
    async fn unsubscribe(
        &self,
        data: &NetRequestGrpcUnsubscribe,
    ) -> Result<NetResponseKind, NetResultStatus> {
        let mut listeners = self.listeners.lock().await;
        let handler = listeners.remove(&data.id());
        let _ = match handler {
            Some(e) => e.cancel(),
            None => (),
        };
        // 4. Return ID to caller
        Ok(NetResponseKind::Grpc(NetResponseGrpc::Unsubscribe(
            NetResponseGrpcUnsubscribe::new(data.id()),
        )))
    }
}
