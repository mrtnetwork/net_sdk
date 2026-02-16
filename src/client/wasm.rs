use tokio::sync::{broadcast, oneshot};

use crate::{
    types::{
        config::{NetConfig, NetHttpHeader},
        error::NetResultStatus,
        response::NetResponseHttp,
    },
    utils::buffer::StreamEncoding,
};

#[async_trait::async_trait(?Send)]
pub trait IClient {
    async fn connect(&self) -> Result<(), NetResultStatus>;
    fn get_config(&self) -> &NetConfig;
}

#[async_trait::async_trait(?Send)]
pub trait IStreamClient: IClient + Send + Sync + 'static {
    async fn send(&self, data: &[u8]) -> Result<(), NetResultStatus>;
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
#[async_trait::async_trait(?Send)]
pub trait IGrpcClient: IClient + Send + Sync {
    /// Send raw bytes
    async fn unary(&self, buffer: &[u8], method_name: &str) -> Result<Vec<u8>, NetResultStatus>;

    /// Send a streaming RPC and receive a broadcast channel for multiple messages
    async fn stream(
        &self,
        buffer: &[u8],
        method_name: &str,
    ) -> Result<GrpcStreamHandle, NetResultStatus>;

    async fn close(&self);
}
#[async_trait::async_trait(?Send)]
pub trait IHttpClient: IClient + Send + Sync {
    async fn send(
        &self,
        url: &str,
        method: &str,
        body: Option<&[u8]>,
        headers: Option<&[NetHttpHeader]>,
        encoding: StreamEncoding,
    ) -> Result<NetResponseHttp, NetResultStatus>;

    async fn close(&self);
}
