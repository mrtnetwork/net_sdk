use crate::{
    client::{http::wasm::HttpClient, wasm::IHttpClient},
    transport::wasm::{IHttpTransport, Transport},
    types::{
        DartCallback,
        config::{NetConfig, NetConfigRequest, NetProtocol},
        error::NetResultStatus,
        request::{NetRequest, NetRequestHttp},
        response::NetResponseKind,
    },
};

pub struct HttpTransport {
    client: Box<dyn IHttpClient>,
    _callback: DartCallback,
    _transport_id: u32,
}
#[async_trait::async_trait(?Send)]
impl Transport for HttpTransport {
    fn create(
        config: NetConfigRequest,
        callback: DartCallback,
        transport_id: u32,
    ) -> Result<Self, NetResultStatus> {
        let config = config.to_protocol_config(NetProtocol::Http)?;
        let client = HttpClient::new(config)?;
        Ok(Self {
            client: Box::new(client),
            _callback: callback,
            _transport_id: transport_id,
        })
    }

    async fn do_request(&self, request: NetRequest) -> Result<NetResponseKind, NetResultStatus> {
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

#[async_trait::async_trait(?Send)]
impl IHttpTransport for HttpTransport {
    async fn send(&self, request: &NetRequestHttp) -> Result<NetResponseKind, NetResultStatus> {
        // self.get_config().e
        let result = self
            .client
            .send(
                request.url(),
                request.method(),
                request.body(),
                request.headers(),
                request.encoding(),
            )
            .await?;
        Ok(NetResponseKind::Http(result))
    }
}
