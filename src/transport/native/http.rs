use arti_client::DataStream;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use crate::{
    client::{
        IHttpClient,
        http::native::{AutoSendRequest, HttpClient},
    },
    transport::{Transport, native::IHttpTransport},
    types::{
        DartCallback,
        config::{NetConfig, NetMode, NetProtocol, NetRequestConfig},
        error::NetResultStatus,
        request::{NetHttpRequest, NetRequest},
        response::NetResponseKind,
    },
};

pub struct HttpTransport {
    client: Box<dyn IHttpClient>,
    _callback: DartCallback,
    _transport_id: u32,
}
#[async_trait::async_trait]
impl Transport for HttpTransport {
    fn create(
        config: NetRequestConfig,
        callback: DartCallback,
        transport_id: u32,
    ) -> Result<Self, NetResultStatus> {
        let config = config.to_protocol_config(NetProtocol::Http)?;
        let client: Box<dyn IHttpClient> = match config.protocol {
            NetProtocol::Http => match (config.addr.is_tls, &config.mode) {
                (true, NetMode::Tor) => {
                    Box::new(HttpClient::<TlsStream<DataStream>, AutoSendRequest>::default(config)?)
                }

                (true, NetMode::Clearnet) => {
                    Box::new(HttpClient::<TlsStream<TcpStream>, AutoSendRequest>::default(config)?)
                }

                (false, NetMode::Tor) => {
                    Box::new(HttpClient::<DataStream, AutoSendRequest>::default(config)?)
                }

                (false, NetMode::Clearnet) => {
                    Box::new(HttpClient::<TcpStream, AutoSendRequest>::default(config)?)
                }
            },
            _ => return Err(NetResultStatus::InvalidConfigParameters),
        };
        Ok(Self {
            client: client,
            _callback: callback,
            _transport_id: transport_id,
        })
    }

    async fn do_request<'a>(
        &self,
        request: NetRequest<'a>,
    ) -> Result<NetResponseKind, NetResultStatus> {
        let http_request = request.to_http_request()?;
        self.send(http_request).await
    }
    async fn close(&self) {
        self.client.close().await;
    }
    fn get_config(&self) -> &NetConfig {
        self.client.get_config()
    }
}

#[async_trait::async_trait]
impl IHttpTransport for HttpTransport {
    async fn send<'a>(
        &self,
        request: &NetHttpRequest<'a>,
    ) -> Result<NetResponseKind, NetResultStatus> {
        // self.get_config().e
        let result = self
            .client
            .send(
                request.url,
                request.method,
                request.body,
                request.headers.as_ref(),
                request.encoding,
            )
            .await?;
        Ok(NetResponseKind::Http(result))
    }
}
