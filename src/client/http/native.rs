use crate::{
    client::{
        http::executor::TokioExecutor,
        native::{IClient, IHttpClient},
    },
    stream::ConnectStream,
    types::{
        config::{NetConfig, NetHttpHeader, NetHttpProtocol},
        error::NetResultStatus,
        native::request::{NetHttpHeaderRef, NetHttpRetryConfig},
        response::NetResponseHttp,
    },
    utils::buffer::{StreamBuffer, StreamEncoding},
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
use log::debug;
use std::{marker::PhantomData, str::FromStr, sync::Arc, time::Duration};
use tokio::{sync::Mutex, time::sleep};

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
        let (sender, connection) =
            hyper::client::conn::http1::handshake(tokio)
                .await
                .map_err(|e| {
                    debug!("HTTP/1 handshake error: {:?}", e);
                    NetResultStatus::ConnectionError
                })?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                debug!("HTTP/1 connection error: {:?}", e);
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
            .map_err(|e| {
                debug!("HTTP/2 handshake error: {:?}", e);
                NetResultStatus::ConnectionError
            })?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                debug!("HTTP/2 connection error: {:?}", e);
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
            Some(NetHttpProtocol::Http2) => {
                if alpn != Some(b"h2") {
                    return Err(NetResultStatus::Http2ConctionFailed);
                }

                let sender = http2::SendRequest::<Full<Bytes>>::connect::<T>(config)
                    .await
                    .map_err(|_| NetResultStatus::ConnectionError)?;

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
                    .map_err(|_| NetResultStatus::ConnectionError)?;

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
                    .map_err(|_| NetResultStatus::ConnectionError)?;

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
    sender: Arc<Mutex<Option<Arc<Mutex<Box<dyn SendRequestExt>>>>>>,
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
    async fn conneect_inner<'a>(&self) -> Result<(), NetResultStatus> {
        let mut guard = self.sender.lock().await;
        let reconnect_needed = match guard.as_mut() {
            Some(_) => false, // sender exists, assume ready
            None => true,
        };

        if reconnect_needed {
            let sender: E = E::connect::<T>(&self.config).await?;
            *guard = Some(Arc::new(Mutex::new(Box::new(sender))));
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
            *guard = Some(Arc::new(Mutex::new(Box::new(sender))));
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
        retry_config: &NetHttpRetryConfig<'a>,
    ) -> Result<NetResponseHttp, NetResultStatus> {
        let method = Method::from_bytes(method.as_bytes()).map_err(|e| {
            debug!("Http invalid method name: {:?}", e);
            NetResultStatus::InvalidRequestParameters
        })?;
        self.request(method, url, body, headers, encoding, retry_config)
            .await
    }
    async fn close(&self) {
        let old_sender = self.sender.lock().await.take();
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
        retry_config: &NetHttpRetryConfig<'a>,
    ) -> Result<NetResponseHttp, NetResultStatus> {
        self.conneect_inner().await?;
        let config = &self.config.http.headers;
        let uri = Uri::from_str(url).unwrap();
        let host = uri
            .host()
            .ok_or(NetResultStatus::ConnectionError)?
            .to_string();
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
                    builder = builder.header(h.key(), h.value());
                }
                builder
            }
        };
        let body = match body {
            Some(b) => Full::new(Bytes::from(b.to_vec())),
            None => Full::new(Bytes::new()),
        };
        if let Some(sender_arc) = self.sender.lock().await.as_ref() {
            let sender = sender_arc.lock().await;
            if sender.protocol() == NetHttpProtocol::Http1 {
                builder = builder.header(http::header::HOST, host.clone());
            }
        }

        let req = builder.body(body).map_err(|_| {
            debug!("Create http body error.",);
            NetResultStatus::InvalidRequestParameters
        })?;
        let retry_delay = Duration::from_millis(retry_config.retry_delay as u64);
        for attempt in 0..=retry_config.max_retries {
            let sender_arc = {
                let guard = self.sender.lock().await;
                guard
                    .as_ref()
                    .ok_or(NetResultStatus::ConnectionError)?
                    .clone()
            };
            // lock inner sender for this request
            let mut sender = sender_arc.lock().await;

            // send request
            let result = sender.send(req.clone()).await;
            match result {
                Ok(resp) => {
                    let status = resp.status().as_u16();

                    if retry_config.retry_status.contains(&status)
                        && attempt < retry_config.max_retries
                    {
                        debug!(
                            "Retrying due to status {} (attempt {})",
                            status,
                            attempt + 1
                        );

                        sleep(retry_delay).await;
                        continue;
                    }

                    return HttpClient::<T, E>::read_response(resp, encoding).await;
                }

                Err(_) => {
                    if attempt >= retry_config.max_retries {
                        return Err(NetResultStatus::ConnectionError);
                    }

                    // Reconnect: acquire lock only while replacing sender
                    let new_sender: Arc<Mutex<Box<dyn SendRequestExt>>> =
                        Arc::new(Mutex::new(Box::new(E::connect::<T>(&self.config).await?)
                            as Box<dyn SendRequestExt>));
                    let mut guard = self.sender.lock().await;
                    *guard = Some(new_sender);

                    sleep(retry_delay).await;
                    continue;
                }
            }
        }

        Err(NetResultStatus::ConnectionError)
    }

    async fn read_response(
        resp: Response<Incoming>,
        encoding: StreamEncoding,
    ) -> Result<NetResponseHttp, NetResultStatus> {
        let status_code = resp.status().as_u16();
        let is_success = (200..300).contains(&status_code);
        // extract headers BEFORE consuming resp
        let headers: Vec<NetHttpHeader> = resp
            .headers()
            .iter()
            .map(|(k, v)| NetHttpHeader::new(k.to_string(), v.to_str().unwrap().to_string()))
            .collect();
        let body = HttpClient::<T, E>::read_body(resp.into_body()).await?;
        let (body, encoding) = match is_success {
            true => StreamBuffer::try_current_buffer(body, encoding),
            false => (body, StreamEncoding::Raw),
        };
        // let headers = resp.headers()
        Ok(NetResponseHttp::new(status_code, body, headers, encoding))
    }
    async fn read_body(mut body: Incoming) -> Result<Vec<u8>, NetResultStatus> {
        let mut out = Vec::new();
        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(|e| {
                debug!("Http read body error: {:?}", e);
                NetResultStatus::InternalError
            })?;
            if let Some(data) = frame.data_ref() {
                out.extend_from_slice(data);
            }
        }

        Ok(out)
    }
}
