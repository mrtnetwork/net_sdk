use std::{
    pin::Pin,
    task::{Context, Poll},
};

use hyper_util::rt::TokioIo;
use tonic::transport::Uri;
use tower::Service;

use crate::{
    stream::ConnectStream,
    types::{
        config::{NetConfig, NetMode, NetProtocol, NetTlsMode},
        error::NetResultStatus,
    },
    utils::{Utils, buffer::StreamEncoding},
};
pub struct GrpcConnector<T> {
    pub tls_mode: NetTlsMode,
    pub _marker: std::marker::PhantomData<T>,
}

impl<T> GrpcConnector<T> {
    pub fn default(config: &NetConfig) -> Self {
        Self {
            _marker: std::marker::PhantomData,
            tls_mode: config.tls_mode,
        }
    }
}

impl<T> Service<Uri> for GrpcConnector<T>
where
    T: ConnectStream,
{
    type Response = TokioIo<T>;
    type Error = NetResultStatus;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Uri) -> Self::Future {
        let tls_mode = self.tls_mode.clone();
        Box::pin(async move {
            let addr = Utils::parse_http_url(&req.to_string())?;
            let config = NetConfig {
                addr,
                mode: NetMode::Clearnet,
                protocol: NetProtocol::Grpc,
                tls_mode: tls_mode,
                http: Default::default(),
                encoding: StreamEncoding::Raw,
            };
            let stream = T::connect(&config).await?;
            Ok(TokioIo::new(stream))
        })
    }
}
