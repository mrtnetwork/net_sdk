use crate::types::{
    DartCallback,
    config::{NetConfig, NetRequestConfig},
    error::NetResultStatus,
    request::NetRequest,
    response::NetResponseKind,
};

pub mod native;

#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    fn create(
        config: NetRequestConfig,
        callback: DartCallback,
        transport_id: u32,
    ) -> Result<Self, NetResultStatus>
    where
        Self: Sized;
    async fn do_request<'a>(
        &self,
        request: NetRequest<'a>,
    ) -> Result<NetResponseKind, NetResultStatus>;

    async fn close(&self);
    fn get_config(&self) -> &NetConfig;
}
