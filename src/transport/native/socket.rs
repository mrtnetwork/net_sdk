use arti_client::DataStream;
use tokio::{
    net::TcpStream,
    sync::{
        Mutex,
        broadcast::{self, Receiver},
    },
};
use tokio_rustls::client::TlsStream;

use crate::{
    client::{
        native::IStreamClient, raw::native::RawStreamClient, websocket::native::WsStreamClient,
    },
    transport::native::{ISocketTransport, Transport},
    types::{
        DartCallback,
        config::{NetConfig, NetConfigRequest, NetMode, NetProtocol},
        error::NetResultStatus,
        native::request::{NetRequest, NetRequestSocket, NetRequestSocketSend},
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
#[async_trait::async_trait]
impl Transport for SocketTransport {
    fn create(
        config: NetConfigRequest,
        callback: DartCallback,
        transport_id: u32,
    ) -> Result<Self, NetResultStatus> {
        let config = config
            .to_protocol_config(NetProtocol::Socket)
            .or_else(|_| config.to_protocol_config(NetProtocol::WebSocket))?;

        let stream: Box<dyn IStreamClient> = match config.protocol {
            NetProtocol::WebSocket => match (config.addr.is_tls, &config.mode) {
                (true, NetMode::Tor) => {
                    Box::new(WsStreamClient::<TlsStream<DataStream>>::default(config)?)
                }

                (true, NetMode::Clearnet) => {
                    Box::new(WsStreamClient::<TlsStream<TcpStream>>::default(config)?)
                }

                (false, NetMode::Tor) => Box::new(WsStreamClient::<DataStream>::default(config)?),

                (false, NetMode::Clearnet) => {
                    Box::new(WsStreamClient::<TcpStream>::default(config)?)
                }
            },
            NetProtocol::Socket => match (config.addr.is_tls, &config.mode) {
                (true, NetMode::Tor) => {
                    Box::new(RawStreamClient::<TlsStream<DataStream>>::default(config)?)
                }

                (true, NetMode::Clearnet) => {
                    Box::new(RawStreamClient::<TlsStream<TcpStream>>::default(config)?)
                }

                (false, NetMode::Tor) => Box::new(RawStreamClient::<DataStream>::default(config)?),

                (false, NetMode::Clearnet) => {
                    Box::new(RawStreamClient::<TcpStream>::default(config)?)
                }
            },
            _ => return Err(NetResultStatus::InvalidConfigParameters),
        };

        Ok(Self {
            stream: stream,
            callback,
            rx: Mutex::new(None),
            _transport_id: transport_id,
        })
    }
    async fn do_request<'a>(
        &self,
        request: NetRequest<'a>,
    ) -> Result<NetResponseKind, NetResultStatus> {
        let socket_requset = request.to_socket_request()?;
        let _ = match socket_requset {
            NetRequestSocket::Subscribe => self.subscribe().await?,
            NetRequestSocket::Unsubscribe => self.unsubscribe().await?,
            NetRequestSocket::Send(socket_request_send) => self.send(socket_request_send).await?,
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
#[async_trait::async_trait]
impl ISocketTransport for SocketTransport {
    async fn send<'a>(&self, data: &NetRequestSocketSend<'a>) -> Result<(), NetResultStatus> {
        self.stream.send(&data.data).await
    }

    async fn subscribe(&self) -> Result<(), NetResultStatus> {
        let mut rx = self.stream.subscribe().await?;
        {
            let mut guard = self.rx.lock().await;
            if guard.is_some() {
                return Ok(());
            }
            *guard = Some(rx.resubscribe()); // store a clone in the struct
        }
        let callback = self.callback.clone();
        let encoding = self.get_config().encoding;
        tokio::spawn(async move {
            let mut buffer = StreamBuffer::new(encoding);
            println!("craete buffer {:#?}", encoding);
            loop {
                match rx.recv().await {
                    Ok(msg) => match msg {
                        Ok(data) => match data {
                            Some(data) => {
                                println!("data send {:#?}", data.len());
                                if let Some(parsed) = buffer.add(data) {
                                    println!("buffer success ${:#?}", parsed.len());
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
