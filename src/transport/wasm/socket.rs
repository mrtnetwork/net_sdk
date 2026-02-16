use tokio::sync::{
    Mutex,
    broadcast::{self, Receiver},
};
use wasm_bindgen_futures::spawn_local;

use crate::{
    client::{wasm::IStreamClient, websocket::wasm::WsStreamClient},
    transport::wasm::{ISocketTransport, Transport},
    types::{
        DartCallback,
        config::{NetConfig, NetConfigRequest, NetProtocol},
        error::NetResultStatus,
        request::{NetRequest, NetRequestSocketSend},
        response::{
            NetResponseKind, NetResponseSocketOk, NetResponseStream, NetResponseStreamData,
            NetResponseStreamError,
        },
    },
    utils::buffer::StreamBuffer,
};

pub struct SocketTransport {
    stream: Box<dyn IStreamClient>,
    callback: DartCallback,
    rx: Mutex<Option<Receiver<Result<Option<Vec<u8>>, NetResultStatus>>>>,
    _transport_id: u32,
}
#[async_trait::async_trait(?Send)]
impl Transport for SocketTransport {
    fn create(
        config: NetConfigRequest,
        callback: DartCallback,
        transport_id: u32,
    ) -> Result<Self, NetResultStatus> {
        let config = config
            .to_protocol_config(NetProtocol::Socket)
            .or_else(|_| config.to_protocol_config(NetProtocol::WebSocket))?;
        let client = WsStreamClient::default(config)?;

        Ok(Self {
            stream: Box::new(client),
            callback,
            rx: Mutex::new(None),
            _transport_id: transport_id,
        })
    }
    async fn do_request(&self, request: NetRequest) -> Result<NetResponseKind, NetResultStatus> {
        let socket_requset = request.to_socket_request()?;
        let _ = match socket_requset {
            crate::types::request::NetRequestSocket::Subscribe => self.subscribe().await?,
            crate::types::request::NetRequestSocket::Unsubscribe => self.unsubscribe().await?,
            crate::types::request::NetRequestSocket::Send(socket_request_send) => {
                self.send(socket_request_send).await?
            }
        };
        Ok(NetResponseKind::Socket(NetResponseSocketOk))
    }

    async fn close(&self) {
        self.stream.close().await;
        let mut guard = self.rx.lock().await;
        if let Some(rx) = guard.take() {
            drop(rx);
        }
        *guard = None;
    }

    fn get_config(&self) -> &NetConfig {
        self.stream.get_config()
    }
}
#[async_trait::async_trait(?Send)]
impl ISocketTransport for SocketTransport {
    async fn send(&self, data: &NetRequestSocketSend) -> Result<(), NetResultStatus> {
        self.stream.send(data.data()).await
    }

    async fn subscribe(&self) -> Result<(), NetResultStatus> {
        // Create a new receiver from the inner RawStreamClient
        let mut rx = self.stream.subscribe().await?;

        // Store it in self.rx
        {
            let mut guard = self.rx.lock().await;
            if guard.is_some() {
                return Ok(());
            }
            *guard = Some(rx.resubscribe()); // store a clone in the struct
        }
        let callback = self.callback.clone();
        let encoding = self.get_config().encoding;
        spawn_local(async move {
            let mut buffer = StreamBuffer::new(encoding);
            loop {
                match rx.recv().await {
                    Ok(msg) => match msg {
                        Ok(data) => match data {
                            Some(data) => {
                                // Try to parse/convert the incoming data
                                if let Some(parsed) = buffer.add(data) {
                                    // Send the processed data to callback
                                    callback(NetResponseKind::Stream(NetResponseStream::Data(
                                        NetResponseStreamData::new(None, parsed),
                                    )));
                                }
                            }
                            None => {
                                callback(NetResponseKind::Stream(NetResponseStream::Close(None)));
                                break;
                            }
                        },
                        Err(err) => {
                            callback(NetResponseKind::Stream(NetResponseStream::Error(
                                NetResponseStreamError::new(None, err),
                            )));
                            break;
                        }
                    },
                    Err(broadcast::error::RecvError::Closed) => {
                        callback(NetResponseKind::Stream(NetResponseStream::Close(None)));
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                }
            }
        });
        Ok(())
    }

    async fn unsubscribe(&self) -> Result<(), NetResultStatus> {
        self.stream.close().await;
        let mut guard = self.rx.lock().await;
        if let Some(rx) = guard.take() {
            drop(rx);
        }
        *guard = None;

        Ok(())
    }
}
