pub mod grpc;
pub mod http;
pub mod socket;
use crate::types::response::NetResponseKind;
use crate::types::{
    DartCallback,
    config::{NetConfig, NetConfigRequest},
    error::NetResultStatus,
    request::{
        NetRequest, NetRequestGrpcStream, NetRequestGrpcUnary, NetRequestGrpcUnsubscribe,
        NetRequestHttp, NetRequestSocketSend,
    },
};
#[async_trait::async_trait(?Send)]
pub trait ISocketTransport {
    /// Send raw bytes
    async fn send(&self, data: &NetRequestSocketSend) -> Result<(), NetResultStatus>;

    /// Subscribe to incoming messages (Dart-style stream)
    async fn subscribe(&self) -> Result<(), NetResultStatus>;

    async fn unsubscribe(&self) -> Result<(), NetResultStatus>;
}
#[async_trait::async_trait(?Send)]
pub trait IGrpcTransport {
    /// Send raw bytes
    async fn unary(&self, data: &NetRequestGrpcUnary) -> Result<NetResponseKind, NetResultStatus>;

    /// Subscribe to incoming messages (Dart-style stream)
    async fn stream(&self, data: &NetRequestGrpcStream)
    -> Result<NetResponseKind, NetResultStatus>;

    async fn unsubscribe(
        &self,
        data: &NetRequestGrpcUnsubscribe,
    ) -> Result<NetResponseKind, NetResultStatus>;
}
#[async_trait::async_trait(?Send)]
pub trait IHttpTransport {
    /// Send raw bytes
    async fn send(&self, request: &NetRequestHttp) -> Result<NetResponseKind, NetResultStatus>;
}
#[async_trait::async_trait(?Send)]
pub trait Transport {
    fn create(
        config: NetConfigRequest,
        callback: DartCallback,
        transport_id: u32,
    ) -> Result<Self, NetResultStatus>
    where
        Self: Sized;
    async fn do_request(&self, request: NetRequest) -> Result<NetResponseKind, NetResultStatus>;

    async fn close(&self);
    fn get_config(&self) -> &NetConfig;
}
