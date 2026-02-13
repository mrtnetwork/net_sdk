use crate::{
    client::{IClient, IHttpClient, http::executor::TokioExecutor},
    stream::ConnectStream,
    types::{
        config::{NetConfig, NetHttpHeader, NetHttpProtocol},
        error::NetResultStatus,
        request::NetHttpHeaderRef,
        response::NetHttpResponse,
    },
    utils::{
        Utils,
        buffer::{StreamBuffer, StreamEncoding},
    },
};
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    Error, Method, Request, Response, Uri,
    body::Incoming,
    client::conn::{http1, http2},
};
use hyper_util::rt::TokioIo;
use log::info;
use std::{marker::PhantomData, str::FromStr, sync::Arc};
use tokio::sync::Mutex;

#[async_trait]
pub trait SendRequestExt: Send + Sync {
    async fn send(
        &mut self,
        req: Request<Full<Bytes>>,
    ) -> Result<Response<hyper::body::Incoming>, Error>;
    fn protocol(&self) -> NetHttpProtocol;
}

// Implement for HTTP/1 SendRequest
#[async_trait]
impl SendRequestExt for http1::SendRequest<Full<Bytes>> {
    async fn send(
        &mut self,
        req: Request<Full<Bytes>>,
    ) -> Result<Response<hyper::body::Incoming>, Error> {
        self.send_request(req).await
    }
    fn protocol(&self) -> NetHttpProtocol {
        NetHttpProtocol::Http1
    }
}

#[async_trait]
impl SendRequestExt for http2::SendRequest<Full<Bytes>> {
    async fn send(
        &mut self,
        req: Request<Full<Bytes>>,
    ) -> Result<Response<hyper::body::Incoming>, Error> {
        self.send_request(req).await
    }
    fn protocol(&self) -> NetHttpProtocol {
        NetHttpProtocol::Http2
    }
}
#[async_trait]
pub trait Connect: Sized {
    async fn connect<T: ConnectStream>(addr: &NetConfig) -> Result<Self, NetResultStatus>;
}
#[async_trait]
impl Connect for http1::SendRequest<Full<Bytes>> {
    async fn connect<T: ConnectStream>(addr: &NetConfig) -> Result<Self, NetResultStatus> {
        let stream = T::connect(addr).await?;
        let tokio = TokioIo::new(stream);
        let (sender, connection) = hyper::client::conn::http1::handshake(tokio)
            .await
            .map_err(|_| NetResultStatus::NetError)?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                info!("HTTP/1 connection error: {:?}", e);
            }
        });
        Ok(sender)
    }
}
#[async_trait]
impl Connect for http2::SendRequest<Full<Bytes>> {
    async fn connect<T: ConnectStream>(addr: &NetConfig) -> Result<Self, NetResultStatus> {
        let stream = T::connect(addr).await?;
        let tokio = TokioIo::new(stream);
        // Builder::new(TokioExecutor).serve_connection(tokio, service_fn(f));
        let (sender, connection) = hyper::client::conn::http2::handshake(TokioExecutor, tokio)
            .await
            .map_err(|_| NetResultStatus::NetError)?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                println!("HTTP/2 connection error: {:?}", e);
            }
        });
        Ok(sender)
    }
}

pub struct AutoSendRequest {
    inner: Box<dyn SendRequestExt>,
}

#[async_trait]
impl Connect for AutoSendRequest {
    async fn connect<T: ConnectStream>(config: &NetConfig) -> Result<Self, NetResultStatus> {
        let stream = T::connect(config).await?;
        let alpn = stream.alpn_protocol();
        let protocol_pref = config.http.protocol.clone();

        match protocol_pref {
            // ===============================
            // Explicit HTTP/2
            // ===============================
            Some(NetHttpProtocol::Http2) => {
                if alpn != Some(b"h2") {
                    return Err(NetResultStatus::Http2ConctionFailed);
                }

                let sender = http2::SendRequest::<Full<Bytes>>::connect::<T>(config)
                    .await
                    .map_err(|_| NetResultStatus::NetError)?;

                Ok(Self {
                    inner: Box::new(sender),
                })
            }

            // ===============================
            // Explicit HTTP/1
            // ===============================
            Some(NetHttpProtocol::Http1) => {
                let sender = http1::SendRequest::<Full<Bytes>>::connect::<T>(config)
                    .await
                    .map_err(|_| NetResultStatus::NetError)?;

                Ok(Self {
                    inner: Box::new(sender),
                })
            }

            // ===============================
            // Auto (try both)
            // ===============================
            None => {
                // Try HTTP/2 first if ALPN allows
                if alpn == Some(b"h2") {
                    if let Ok(sender) =
                        http2::SendRequest::<Full<Bytes>>::connect::<T>(config).await
                    {
                        return Ok(Self {
                            inner: Box::new(sender),
                        });
                    }
                }

                // Fallback to HTTP/1
                let sender = http1::SendRequest::<Full<Bytes>>::connect::<T>(config)
                    .await
                    .map_err(|_| NetResultStatus::NetError)?;

                Ok(Self {
                    inner: Box::new(sender),
                })
            }
        }
    }
}
#[async_trait]
impl SendRequestExt for AutoSendRequest {
    async fn send(
        &mut self,
        req: Request<Full<Bytes>>,
    ) -> Result<Response<hyper::body::Incoming>, Error> {
        self.inner.send(req).await
    }
    fn protocol(&self) -> NetHttpProtocol {
        self.inner.protocol()
    }
}
pub struct HttpClient<T, E> {
    sender: Arc<Mutex<Option<Box<dyn SendRequestExt>>>>,
    _stream_marker: PhantomData<T>,
    _protocol_marker: PhantomData<E>,
    config: NetConfig,
}
impl<T, E> HttpClient<T, E>
where
    T: ConnectStream,
    E: SendRequestExt + Connect + 'static,
{
    pub fn default(config: NetConfig) -> Result<Self, NetResultStatus> {
        Ok(Self {
            sender: Arc::new(Mutex::new(None)),
            _stream_marker: PhantomData,
            _protocol_marker: PhantomData,
            config: config,
        })
    }
    async fn conneect_inner<'a>(&self, url: &'a str) -> Result<(), NetResultStatus> {
        let addr = Utils::parse_http_url(url)?;
        let config_addr = &self.config.addr;
        let mut host_changed = false;
        if addr.host != config_addr.host || addr.port != config_addr.port {
            if !self.config.http.global_client {
                return Err(NetResultStatus::MismatchHttpUrl);
            }
            host_changed = true;
        }
        let mut guard = self.sender.lock().await;

        let reconnect_needed = match guard.as_mut() {
            Some(_) => host_changed, // sender exists, assume ready
            None => true,
        };

        if reconnect_needed {
            let sender: E = match host_changed {
                true => {
                    let new_config = self.config.change_addr(addr);
                    E::connect::<T>(&new_config).await?
                }
                false => E::connect::<T>(&self.config).await?,
            };
            *guard = Some(Box::new(sender));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl<'a, T, E> IClient for HttpClient<T, E>
where
    T: ConnectStream,
    E: SendRequestExt + Connect + 'static,
{
    async fn connect(&self) -> Result<(), NetResultStatus> {
        let mut guard = self.sender.lock().await;

        let reconnect_needed = match guard.as_mut() {
            Some(_) => false, // sender exists, assume ready
            None => true,
        };

        if reconnect_needed {
            let sender: E = E::connect::<T>(&self.config).await?;
            *guard = Some(Box::new(sender));
        }

        Ok(())
    }

    fn get_config(&self) -> &NetConfig {
        &self.config
    }
}

#[async_trait::async_trait]
impl<T, E> IHttpClient for HttpClient<T, E>
where
    T: ConnectStream,
    E: SendRequestExt + Connect + 'static,
{
    async fn send<'a>(
        &self,
        url: &'a str,
        method: &'a str,
        body: Option<&'a [u8]>,
        headers: Option<&Vec<NetHttpHeaderRef<'a>>>,
        encoding: StreamEncoding,
    ) -> Result<NetHttpResponse, NetResultStatus> {
        let method = Method::from_bytes(method.as_bytes())
            .map_err(|_| NetResultStatus::InvalidRequestParameters)?;
        self.request(method, url, body, headers, encoding).await
    }
    async fn close(&self) {
        // Take ownership and remove from Option
        let old_sender = self.sender.lock().await.take();
        // Dropping closes the stream
        drop(old_sender);
    }
}

impl<T, E> HttpClient<T, E>
where
    T: ConnectStream,
    E: SendRequestExt + Connect + 'static,
{
    async fn request<'a>(
        &self,
        method: Method,
        url: &'a str,
        body: Option<&'a [u8]>,
        headers: Option<&Vec<NetHttpHeaderRef<'a>>>,
        encoding: StreamEncoding,
    ) -> Result<NetHttpResponse, NetResultStatus> {
        self.conneect_inner(url).await?;
        let config = &self.config.http.headers;
        let uri = Uri::from_str(url).unwrap();
        let host = uri.host().ok_or(NetResultStatus::NetError)?.to_string();
        let mut builder = Request::builder().method(method).uri(uri);
        builder = match headers {
            Some(headers) => {
                for h in headers {
                    builder = builder.header(h.key.to_string(), h.value.to_string());
                }
                builder
            }
            None => {
                for h in config {
                    builder = builder.header(h.key.to_string(), h.value.to_string());
                }
                builder
            }
        };
        let body = match body {
            Some(b) => Full::new(Bytes::from(b.to_vec())),
            None => Full::new(Bytes::new()),
        };
        let mut guard = self.sender.lock().await;
        let sender = guard.as_mut().ok_or(NetResultStatus::NetError)?;
        let protocol = sender.protocol();
        match protocol {
            NetHttpProtocol::Http1 => {
                builder = builder.header(http::header::HOST, host);
            }
            NetHttpProtocol::Http2 => {}
        };
        let req = builder
            .body(body)
            .map_err(|_| NetResultStatus::InvalidRequestParameters)?;
        match sender.send(req.clone()).await {
            Ok(resp) => HttpClient::<T, E>::read_response(resp, encoding).await,
            Err(e) => {
                // reconnect and retry once
                println!("Connection dead, reconnecting {:#?}", e);
                *guard = Some(Box::new(E::connect::<T>(&self.config).await?));
                let sender = guard.as_mut().unwrap();
                let resp = sender
                    .send(req)
                    .await
                    .map_err(|_| NetResultStatus::NetError)?;
                HttpClient::<T, E>::read_response(resp, encoding).await
            }
        }
    }

    async fn read_response(
        resp: Response<Incoming>,
        encoding: StreamEncoding,
    ) -> Result<NetHttpResponse, NetResultStatus> {
        let status_code = resp.status().as_u16();
        let is_success = (200..300).contains(&status_code);
        // extract headers BEFORE consuming resp
        let headers: Vec<NetHttpHeader> = resp
            .headers()
            .iter()
            .map(|(k, v)| NetHttpHeader {
                key: k.to_string(),
                value: v.to_str().unwrap_or_default().to_string(),
            })
            .collect();
        let body = HttpClient::<T, E>::read_body(resp.into_body()).await?;
        let (body, encoding) = match is_success {
            true => StreamBuffer::try_current_buffer(body, encoding),
            false => (body, StreamEncoding::Raw),
        };
        // let headers = resp.headers()
        Ok(NetHttpResponse {
            status_code,
            body,
            headers,
            encoding,
        })
    }
    async fn read_body(mut body: Incoming) -> Result<Vec<u8>, NetResultStatus> {
        let mut out = Vec::new();
        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(|_| NetResultStatus::NetError)?;
            if let Some(data) = frame.data_ref() {
                out.extend_from_slice(data);
            }
        }

        Ok(out)
    }
}
