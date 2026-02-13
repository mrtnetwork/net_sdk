use crate::types::{
    error::NetResultStatus,
    request::{
        NetGrpcRequestStream, NetGrpcRequestUnary, NetGrpcRequestUnsubscribe, NetHttpRequest,
        NetSocketRequestSend,
    },
    response::NetResponseKind,
};

pub mod grpc;
pub mod http;
pub mod socket;

#[async_trait::async_trait]
pub trait ISocketTransport {
    /// Send raw bytes
    async fn send<'a>(&self, data: &NetSocketRequestSend<'a>) -> Result<(), NetResultStatus>;

    /// Subscribe to incoming messages (Dart-style stream)
    async fn subscribe(&self) -> Result<(), NetResultStatus>;

    async fn unsubscribe(&self) -> Result<(), NetResultStatus>;
}
#[async_trait::async_trait]
pub trait IGrpcTransport<'a> {
    /// Send raw bytes
    async fn unary(
        &self,
        data: &NetGrpcRequestUnary<'a>,
    ) -> Result<NetResponseKind, NetResultStatus>;

    /// Subscribe to incoming messages (Dart-style stream)
    async fn stream(
        &self,
        data: &NetGrpcRequestStream<'a>,
    ) -> Result<NetResponseKind, NetResultStatus>;

    async fn unsubscribe(
        &self,
        data: &NetGrpcRequestUnsubscribe,
    ) -> Result<NetResponseKind, NetResultStatus>;
}
#[async_trait::async_trait]
pub trait IHttpTransport {
    /// Send raw bytes
    async fn send<'a>(
        &self,
        request: &NetHttpRequest<'a>,
    ) -> Result<NetResponseKind, NetResultStatus>;
}
