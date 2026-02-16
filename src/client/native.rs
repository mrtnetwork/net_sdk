use tokio::sync::{broadcast, oneshot};

use crate::{
    types::{
        config::NetConfig,
        error::NetResultStatus,
        native::request::{NetHttpHeaderRef, NetHttpRetryConfig},
        response::NetResponseHttp,
    },
    utils::buffer::StreamEncoding,
};

#[async_trait::async_trait]
pub trait IClient {
    async fn connect(&self) -> Result<(), NetResultStatus>;
    fn get_config(&self) -> &NetConfig;
}

#[async_trait::async_trait]
pub trait IStreamClient: IClient + Send + Sync + 'static {
    async fn send<'a>(&self, data: &'a [u8]) -> Result<(), NetResultStatus>;
    async fn subscribe(
        &self,
    ) -> Result<broadcast::Receiver<Result<Option<Vec<u8>>, NetResultStatus>>, NetResultStatus>;

    async fn close(&self);
}

pub struct GrpcStreamHandle {
    pub rx: broadcast::Receiver<Result<Option<Vec<u8>>, NetResultStatus>>,
    pub cancel: oneshot::Sender<()>,
}
impl GrpcStreamHandle {
    pub fn cancel(self) {
        let _ = self.cancel.send(());
    }
}
#[async_trait::async_trait]
pub trait IGrpcClient: IClient + Send + Sync {
    async fn unary<'a>(
        &self,
        buffer: &'a [u8],
        method_name: &'a str,
    ) -> Result<Vec<u8>, NetResultStatus>;

    async fn stream<'a>(
        &self,
        buffer: &'a [u8],
        method_name: &'a str,
    ) -> Result<GrpcStreamHandle, NetResultStatus>;

    async fn close(&self);
}
#[async_trait::async_trait]
pub trait IHttpClient: IClient + Send + Sync {
    async fn send<'a>(
        &self,
        url: &'a str,
        method: &'a str,
        body: Option<&'a [u8]>,
        headers: Option<&Vec<NetHttpHeaderRef<'a>>>,
        encoding: StreamEncoding,
        retry_config: &NetHttpRetryConfig<'a>,
    ) -> Result<NetResponseHttp, NetResultStatus>;

    async fn close(&self);
}
