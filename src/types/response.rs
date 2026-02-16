use crate::{
    types::{config::NetHttpHeader, error::NetResultStatus},
    utils::buffer::StreamEncoding,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[derive(Debug)]
pub struct NetResponseSocketOk;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug)]
pub struct NetResponseStreamData {
    id: Option<i32>,
    data: Vec<u8>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetResponseStreamData {
    /// Getter for `id`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn id(&self) -> Option<i32> {
        self.id
    }
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn data(&self) -> Vec<u8> {
        self.data.clone() // clone so JS owns its copy
    }
}

impl NetResponseStreamData {
    pub fn new(id: Option<i32>, data: Vec<u8>) -> NetResponseStreamData {
        Self { id, data }
    }
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug)]
pub struct NetResponseStreamError {
    id: Option<i32>,
    status: NetResultStatus,
}

impl NetResponseStreamError {
    pub fn new(id: Option<i32>, status: NetResultStatus) -> NetResponseStreamError {
        Self { id, status }
    }
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetResponseStreamError {
    /// Getter for `id`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn id(&self) -> Option<i32> {
        self.id
    }

    /// Getter for `status`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn status(&self) -> NetResultStatus {
        self.status.clone()
    }
}
#[derive(Debug)]
pub enum NetResponseStream {
    Data(NetResponseStreamData),
    Close(Option<i32>),
    Error(NetResponseStreamError),
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug)]
pub struct NetResponseHttp {
    status_code: u16,
    body: Vec<u8>,
    headers: Vec<NetHttpHeader>,
    encoding: StreamEncoding,
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetResponseHttp {
    /// Getter for `status_code`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn status_code(&self) -> u16 {
        self.status_code
    }

    /// Getter for `body`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn body(&self) -> Vec<u8> {
        self.body.clone()
    }

    /// Getter for `headers`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn headers(&self) -> Vec<NetHttpHeader> {
        self.headers.clone()
    }

    /// Getter for `encoding`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn encoding(&self) -> StreamEncoding {
        self.encoding.clone()
    }
}
impl NetResponseHttp {
    pub fn new(
        status_code: u16,
        body: Vec<u8>,
        headers: Vec<NetHttpHeader>,
        encoding: StreamEncoding,
    ) -> NetResponseHttp {
        Self {
            status_code,
            body,
            headers,
            encoding,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug)]
pub struct NetResponseGrpcSubscribe {
    id: i32,
}

impl NetResponseGrpcSubscribe {
    pub fn new(id: i32) -> NetResponseGrpcSubscribe {
        Self { id }
    }
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetResponseGrpcSubscribe {
    /// Getter for `id`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn id(&self) -> i32 {
        self.id
    }
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug)]
pub struct NetResponseGrpcUnsubscribe {
    id: i32,
}
impl NetResponseGrpcUnsubscribe {
    pub fn new(id: i32) -> NetResponseGrpcUnsubscribe {
        Self { id }
    }
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetResponseGrpcUnsubscribe {
    /// Getter for `id`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn id(&self) -> i32 {
        self.id
    }
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug)]
pub struct NetResponseGrpcUnary {
    data: Vec<u8>,
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetResponseGrpcUnary {
    /// Getter for `data`
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn data(&self) -> Vec<u8> {
        self.data.clone() // clone so JS owns its own copy
    }
}
impl NetResponseGrpcUnary {
    pub fn new(data: Vec<u8>) -> NetResponseGrpcUnary {
        Self { data }
    }
}
#[derive(Debug)]
pub enum NetResponseGrpc {
    Unary(NetResponseGrpcUnary),
    StreamId(NetResponseGrpcSubscribe),
    Unsubscribe(NetResponseGrpcUnsubscribe),
}

pub struct NetResponse {
    pub transport_id: u32,
    pub request_id: u32,
    pub response: NetResponseKind,
}
#[derive(Debug)]
pub enum NetResponseKind {
    Socket(NetResponseSocketOk),
    Grpc(NetResponseGrpc),
    Http(NetResponseHttp),
    Stream(NetResponseStream),
    ResponseError(NetResultStatus),
    TransportClosed,
    TorInited(bool),
}
impl NetResponseKind {
    pub fn grpc_unary(&self) -> Option<NetResponseGrpcUnary> {
        match self {
            NetResponseKind::Grpc(net_grpc_response) => match net_grpc_response {
                NetResponseGrpc::Unary(net_grpc_unary_response) => {
                    Some(net_grpc_unary_response.clone())
                }
                _ => None,
            },
            _ => None,
        }
    }
    pub fn grpc_stream_id(&self) -> Option<NetResponseGrpcSubscribe> {
        match self {
            NetResponseKind::Grpc(net_grpc_response) => match net_grpc_response {
                NetResponseGrpc::StreamId(net_grpc_stream_id_response) => {
                    Some(net_grpc_stream_id_response.clone())
                }
                _ => None,
            },
            _ => None,
        }
    }
    pub fn grpc_unsubscribe(&self) -> Option<NetResponseGrpcUnsubscribe> {
        match self {
            NetResponseKind::Grpc(net_grpc_response) => match net_grpc_response {
                NetResponseGrpc::Unsubscribe(net_grpc_unsubscribe_stream_id_response) => {
                    Some(net_grpc_unsubscribe_stream_id_response.clone())
                }
                _ => None,
            },
            _ => None,
        }
    }
    pub fn stream_data(&self) -> Option<NetResponseStreamData> {
        match self {
            NetResponseKind::Stream(net_stream_response) => match net_stream_response {
                NetResponseStream::Data(net_stream_response_data) => {
                    Some(net_stream_response_data.clone())
                }
                _ => None,
            },
            _ => None,
        }
    }
    pub fn stream_close(&self) -> Option<i32> {
        match self {
            NetResponseKind::Stream(net_stream_response) => match net_stream_response {
                NetResponseStream::Close(e) => Some(e.map_or(-1, |f| f)),
                _ => None,
            },
            _ => None,
        }
    }
    pub fn stream_error(&self) -> Option<NetResponseStreamError> {
        match self {
            NetResponseKind::Stream(net_stream_response) => match net_stream_response {
                NetResponseStream::Error(net_stream_response_error) => {
                    Some(net_stream_response_error.clone())
                }
                _ => None,
            },
            _ => None,
        }
    }
    pub fn http(&self) -> Option<NetResponseHttp> {
        match self {
            NetResponseKind::Http(net_http_response) => Some(net_http_response.clone()),
            _ => None,
        }
    }
    pub fn error(&self) -> Option<NetResultStatus> {
        match self {
            NetResponseKind::ResponseError(net_result_status) => Some(net_result_status.clone()),
            _ => None,
        }
    }
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]

pub struct NetResponseWasm {
    transport_id: u32,
    request_id: u32,
    kind: u8,
    grpc_unary: Option<NetResponseGrpcUnary>,
    grpc_stream: Option<NetResponseGrpcSubscribe>,
    grpc_unsubscribe: Option<NetResponseGrpcUnsubscribe>,
    http: Option<NetResponseHttp>,
    stream_data: Option<NetResponseStreamData>,
    stream_close: Option<i32>,
    stream_error: Option<NetResponseStreamError>,
    response_error: Option<NetResultStatus>,
}
impl NetResponseWasm {
    pub fn from_native(reseponse: NetResponse) -> NetResponseWasm {
        Self {
            transport_id: reseponse.transport_id,
            request_id: reseponse.request_id,
            kind: match &reseponse.response {
                NetResponseKind::Socket(_) => 1,
                NetResponseKind::Grpc(net_grpc_response) => match net_grpc_response {
                    NetResponseGrpc::Unary(_) => 2,
                    NetResponseGrpc::StreamId(_) => 3,
                    NetResponseGrpc::Unsubscribe(_) => 4,
                },
                NetResponseKind::Http(_) => 5,
                NetResponseKind::Stream(net_stream_response) => match net_stream_response {
                    NetResponseStream::Data(_) => 6,
                    NetResponseStream::Close(_) => 7,
                    NetResponseStream::Error(_) => 8,
                },
                NetResponseKind::ResponseError(_) => 9,
                NetResponseKind::TransportClosed => 10,
                NetResponseKind::TorInited(_) => 11,
            },
            grpc_unary: reseponse.response.grpc_unary(),
            grpc_stream: reseponse.response.grpc_stream_id(),
            grpc_unsubscribe: reseponse.response.grpc_unsubscribe(),
            http: reseponse.response.http(),
            stream_data: reseponse.response.stream_data(),
            stream_close: reseponse.response.stream_close(),
            stream_error: reseponse.response.stream_error(),
            response_error: reseponse.response.error(),
        }
    }
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetResponseWasm {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn transport_id(&self) -> u32 {
        self.transport_id
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn request_id(&self) -> u32 {
        self.request_id
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn kind(&self) -> u8 {
        self.kind
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn grpc_unary(&self) -> Option<NetResponseGrpcUnary> {
        self.grpc_unary.clone()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn grpc_stream(&self) -> Option<NetResponseGrpcSubscribe> {
        self.grpc_stream.clone()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn grpc_unsubscribe(&self) -> Option<NetResponseGrpcUnsubscribe> {
        self.grpc_unsubscribe.clone()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn http(&self) -> Option<NetResponseHttp> {
        self.http.clone()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn stream_data(&self) -> Option<NetResponseStreamData> {
        self.stream_data.clone()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn stream_close(&self) -> Option<i32> {
        self.stream_close
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn stream_error(&self) -> Option<NetResponseStreamError> {
        self.stream_error.clone()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn response_error(&self) -> Option<NetResultStatus> {
        self.response_error.clone()
    }
}
