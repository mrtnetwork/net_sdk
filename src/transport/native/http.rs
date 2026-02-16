use arti_client::DataStream;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use crate::{
    client::{
        http::native::{AutoSendRequest, HttpClient},
        native::IHttpClient,
    },
    transport::native::{IHttpTransport, Transport},
    types::{
        DartCallback,
        config::{NetConfig, NetConfigRequest, NetMode, NetProtocol},
        error::NetResultStatus,
        native::request::{NetRequest, NetRequestHttp},
        response::NetResponseKind,
    },
    utils::Utils,
};

pub struct HttpTransport {
    client: Box<dyn IHttpClient>,
    _callback: DartCallback,
    _transport_id: u32,
}
impl HttpTransport {
    fn create_client(config: NetConfig) -> Result<Box<dyn IHttpClient>, NetResultStatus> {
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
        Ok(client)
    }
}
#[async_trait::async_trait]
impl Transport for HttpTransport {
    fn create(
        config: NetConfigRequest,
        callback: DartCallback,
        transport_id: u32,
    ) -> Result<Self, NetResultStatus> {
        let config = config.to_protocol_config(NetProtocol::Http)?;
        let client = HttpTransport::create_client(config)?;
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
        let addr = Utils::parse_http_url(http_request.url)?;
        let config = self.client.get_config();
        if addr.host != config.addr.host {
            let new_config = config.change_addr(addr);
            let client = HttpTransport::create_client(new_config)?;
            let result = client
                .send(
                    &http_request.url,
                    &http_request.method,
                    http_request.body,
                    http_request.headers.as_ref(),
                    http_request.encoding,
                    &http_request.retry_config,
                )
                .await?;
            return Ok(NetResponseKind::Http(result));
        }
        // if(http_request.)
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
        request: &NetRequestHttp<'a>,
    ) -> Result<NetResponseKind, NetResultStatus> {
        let result = self
            .client
            .send(
                &request.url,
                &request.method,
                request.body,
                request.headers.as_ref(),
                request.encoding,
                &request.retry_config,
            )
            .await?;
        Ok(NetResponseKind::Http(result))
    }
}
