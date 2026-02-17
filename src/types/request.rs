use crate::{
    types::{
        config::{NetHttpHeader, NetProtocol},
        error::NetResultStatus,
    },
    utils::buffer::StreamEncoding,
};
use wasm_bindgen::prelude::*;

#[derive(Clone)]
#[wasm_bindgen]
pub struct NetRequestGrpcUnary {
    method: String,
    data: Vec<u8>,
}
#[wasm_bindgen]
impl NetRequestGrpcUnary {
    #[wasm_bindgen]
    pub fn create(method: String, data: Vec<u8>) -> Self {
        Self { method, data }
    }
}
impl NetRequestGrpcUnary {
    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct NetRequestGrpcStream {
    method: String,
    data: Vec<u8>,
}
#[wasm_bindgen]
impl NetRequestGrpcStream {
    #[wasm_bindgen]
    pub fn create(method: String, data: Vec<u8>) -> Self {
        Self { method, data }
    }
}
impl NetRequestGrpcStream {
    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct NetRequestGrpcUnsubscribe {
    id: i32,
}
#[wasm_bindgen]
impl NetRequestGrpcUnsubscribe {
    #[wasm_bindgen]
    pub fn create(id: i32) -> Self {
        Self { id }
    }
}

impl NetRequestGrpcUnsubscribe {
    pub fn id(&self) -> i32 {
        self.id
    }
}

#[derive(Clone)]

pub enum NetRequestGrpc {
    Unary(NetRequestGrpcUnary),
    Stream(NetRequestGrpcStream),
    Unsubscribe(NetRequestGrpcUnsubscribe),
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct NetRequestHttp {
    method: String,
    url: String,
    body: Option<Vec<u8>>,
    headers: Option<Vec<NetHttpHeader>>,
    encoding: StreamEncoding,
    retry: NetHttpRetryConfig,
}
#[wasm_bindgen]
impl NetRequestHttp {
    #[wasm_bindgen]
    pub fn create(
        method: String,
        url: String,
        body: Option<Vec<u8>>,
        headers: Option<Vec<NetHttpHeader>>,
        encoding: StreamEncoding,
        retry: NetHttpRetryConfig,
    ) -> Self {
        Self {
            method,
            url,
            body,
            headers,
            encoding,
            retry,
        }
    }
}
#[wasm_bindgen]
#[derive(Clone)]
pub struct NetRequestSocketSend {
    data: Vec<u8>,
}
#[wasm_bindgen]
impl NetRequestSocketSend {
    #[wasm_bindgen]
    pub fn create(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl NetRequestSocketSend {
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}
#[wasm_bindgen]
#[derive(Clone)]
pub struct NetHttpRetryConfig {
    max_retries: u8,
    retry_status: Vec<u16>,
    retry_delay: u32,
}
#[wasm_bindgen]
impl NetHttpRetryConfig {
    #[wasm_bindgen]
    pub fn create(max_retries: u8, retry_status: Vec<u16>, retry_delay: u32) -> Self {
        Self {
            max_retries,
            retry_status,
            retry_delay,
        }
    }
}
impl NetHttpRetryConfig {
    pub fn max_retries(&self) -> u8 {
        return self.max_retries;
    }
    pub fn retry_status(&self) -> &Vec<u16> {
        return &self.retry_status;
    }
    pub fn retry_delay(&self) -> u32 {
        return self.retry_delay;
    }
}

#[derive(Clone)]
pub enum NetRequestSocket {
    Subscribe,
    Unsubscribe,
    Send(NetRequestSocketSend),
}

impl NetRequestSocket {
    pub fn send(&self) -> Option<&NetRequestSocketSend> {
        if let NetRequestSocket::Send(inner) = self {
            Some(inner)
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub enum NetRequestKind {
    Socket(NetRequestSocket),
    Grpc(NetRequestGrpc),
    Http(NetRequestHttp),
}
#[wasm_bindgen]
#[derive(Clone)]
pub struct NetRequestWasm {
    transport_id: u32,
    id: u32,
    timeout: u32,
    kind: u8,
    soket_send: Option<NetRequestSocketSend>,
    grpc_unary: Option<NetRequestGrpcUnary>,
    gprc_stream: Option<NetRequestGrpcStream>,
    grpc_unsubscribe: Option<NetRequestGrpcUnsubscribe>,
    http: Option<NetRequestHttp>,
}
#[wasm_bindgen]
impl NetRequestWasm {
    pub fn create(
        transport_id: u32,
        id: u32,
        timeout: u32,
        kind: u8,
        soket_send: Option<NetRequestSocketSend>,
        grpc_unary: Option<NetRequestGrpcUnary>,
        gprc_stream: Option<NetRequestGrpcStream>,
        grpc_unsubscribe: Option<NetRequestGrpcUnsubscribe>,
        http: Option<NetRequestHttp>,
    ) -> NetRequestWasm {
        Self {
            transport_id,
            id,
            timeout,
            kind,
            soket_send,
            grpc_unary,
            gprc_stream,
            grpc_unsubscribe,
            http,
        }
    }
}
impl NetRequestWasm {
    /// Convert WASM-friendly struct to native NetRequest
    pub fn to_native(&self) -> Result<NetRequest, NetResultStatus> {
        // Determine the NetRequestKind based on self.kind
        let kind = match self.kind {
            1 => {
                // Socket
                let socket_send = self
                    .soket_send
                    .as_ref()
                    .ok_or(NetResultStatus::InvalidRequestParameters)?;
                NetRequestKind::Socket(NetRequestSocket::Send(socket_send.clone()))
            }
            2 => NetRequestKind::Socket(NetRequestSocket::Subscribe),
            3 => NetRequestKind::Socket(NetRequestSocket::Unsubscribe),
            4 => {
                // Grpc Unary
                let grpc_unary = self
                    .grpc_unary
                    .as_ref()
                    .ok_or(NetResultStatus::InvalidRequestParameters)?;
                NetRequestKind::Grpc(NetRequestGrpc::Unary(grpc_unary.clone()))
            }
            5 => {
                // Grpc Stream
                let grpc_stream = self
                    .gprc_stream
                    .as_ref()
                    .ok_or(NetResultStatus::InvalidRequestParameters)?;
                NetRequestKind::Grpc(NetRequestGrpc::Stream(grpc_stream.clone()))
            }
            6 => {
                // Grpc Unsubscribe
                let grpc_unsub = self
                    .grpc_unsubscribe
                    .as_ref()
                    .ok_or(NetResultStatus::InvalidRequestParameters)?;
                NetRequestKind::Grpc(NetRequestGrpc::Unsubscribe(grpc_unsub.clone()))
            }
            7 => {
                // Http
                let http = self
                    .http
                    .as_ref()
                    .ok_or(NetResultStatus::InvalidRequestParameters)?;
                NetRequestKind::Http(http.clone())
            }
            _ => return Err(NetResultStatus::InvalidRequestParameters),
        };

        Ok(NetRequest {
            transport_id: self.transport_id,
            id: self.id,
            timeout: self.timeout,
            kind,
        })
    }
}

#[derive(Clone)]
pub struct NetRequest {
    transport_id: u32,
    id: u32,
    timeout: u32,
    kind: NetRequestKind,
}

impl NetRequest {
    pub fn new(transport_id: u32, id: u32, timeout: u32, kind: NetRequestKind) -> Self {
        Self {
            transport_id,
            id,
            timeout,
            kind,
        }
    }

    pub fn transport_id(&self) -> u32 {
        self.transport_id
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn timeout(&self) -> u32 {
        self.timeout
    }

    pub fn kind(&self) -> &NetRequestKind {
        &self.kind
    }
}

impl NetRequest {
    pub fn to_http_request(&self) -> Result<&NetRequestHttp, NetResultStatus> {
        match &self.kind {
            NetRequestKind::Http(http_request) => Ok(http_request),
            _ => Err(NetResultStatus::InvalidRequestParameters),
        }
    }
    pub fn to_socket_request(&self) -> Result<&NetRequestSocket, NetResultStatus> {
        match &self.kind {
            NetRequestKind::Socket(socket_request) => Ok(socket_request),
            _ => Err(NetResultStatus::InvalidRequestParameters),
        }
    }

    pub fn to_grpc_request(&self) -> Result<&NetRequestGrpc, NetResultStatus> {
        match &self.kind {
            NetRequestKind::Grpc(grpc_request) => Ok(grpc_request),
            _ => Err(NetResultStatus::InvalidRequestParameters),
        }
    }
    pub fn to_protocol_config(&self, protocol: NetProtocol) -> Result<(), NetResultStatus> {
        let _ = match protocol {
            NetProtocol::Http => self.to_http_request().map(|_| ())?,
            NetProtocol::Grpc => self.to_grpc_request().map(|_| ())?,
            NetProtocol::WebSocket | NetProtocol::Socket => self.to_socket_request().map(|_| ())?,
        };
        Ok(())
    }
}
impl NetRequestHttp {
    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    pub fn headers(&self) -> Option<&[NetHttpHeader]> {
        self.headers.as_deref()
    }

    pub fn encoding(&self) -> StreamEncoding {
        self.encoding
    }
    pub fn retry_config(&self) -> &NetHttpRetryConfig {
        &self.retry
    }
}
