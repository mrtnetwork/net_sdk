use std::fmt;

use crate::{
    types::{
        config::{NetConfigTor, NetProtocol},
        error::NetResultStatus,
    },
    utils::buffer::StreamEncoding,
};

pub struct NetRequestGrpcUnary<'a> {
    pub method: &'a str,
    pub data: &'a [u8],
}

pub struct NetRequestGrpcStream<'a> {
    pub method: &'a str,
    pub data: &'a [u8],
}

pub struct NetRequestGrpcUnsubscribe {
    pub id: i32,
}
pub struct NetHttpHeaderRef<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

pub enum NetRequestGrpc<'a> {
    Unary(NetRequestGrpcUnary<'a>),
    Stream(NetRequestGrpcStream<'a>),
    Unsubscribe(NetRequestGrpcUnsubscribe),
}

pub struct NetRequestHttp<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub body: Option<&'a [u8]>,
    pub headers: Option<Vec<NetHttpHeaderRef<'a>>>,
    pub encoding: StreamEncoding,
    pub retry_config: NetHttpRetryConfig<'a>,
}

pub struct NetRequestSocketSend<'a> {
    pub data: &'a [u8],
}

pub enum NetRequestSocket<'a> {
    Subscribe,
    Unsubscribe,
    Send(NetRequestSocketSend<'a>),
}

pub enum NetRequestKind<'a> {
    Socket(NetRequestSocket<'a>),
    Grpc(NetRequestGrpc<'a>),
    Http(NetRequestHttp<'a>),
    InitTor(NetConfigTor),
    TorInited,
}

pub struct NetRequest<'a> {
    pub transport_id: u32,
    pub id: u32,
    pub timeout: u32,
    pub kind: NetRequestKind<'a>,
}

pub struct NetHttpRetryConfig<'a> {
    pub max_retries: u8,
    pub retry_status: &'a [u16],
    pub retry_delay: u32,
}

impl<'a> NetHttpRetryConfig<'a> {
    pub fn default() -> NetHttpRetryConfig<'a> {
        Self {
            max_retries: 1,
            retry_status: &[],
            retry_delay: 0,
        }
    }
}

impl<'a> NetRequest<'a> {
    pub fn to_http_request(&'a self) -> Result<&'a NetRequestHttp<'a>, NetResultStatus> {
        match &self.kind {
            NetRequestKind::Http(http_request) => Ok(http_request),
            _ => Err(NetResultStatus::InvalidRequestParameters),
        }
    }
    pub fn to_socket_request(&'a self) -> Result<&'a NetRequestSocket<'a>, NetResultStatus> {
        match &self.kind {
            NetRequestKind::Socket(socket_request) => Ok(socket_request),
            _ => Err(NetResultStatus::InvalidRequestParameters),
        }
    }

    pub fn to_grpc_request(&'a self) -> Result<&'a NetRequestGrpc<'a>, NetResultStatus> {
        match &self.kind {
            NetRequestKind::Grpc(grpc_request) => Ok(grpc_request),
            _ => Err(NetResultStatus::InvalidRequestParameters),
        }
    }
    pub fn to_protocol_config(&'a self, protocol: NetProtocol) -> Result<(), NetResultStatus> {
        let _ = match protocol {
            NetProtocol::Http => self.to_http_request().map(|_| ())?,
            NetProtocol::Grpc => self.to_grpc_request().map(|_| ())?,
            NetProtocol::WebSocket | NetProtocol::Socket => self.to_socket_request().map(|_| ())?,
        };
        Ok(())
    }
}
impl<'a> fmt::Debug for NetRequestKind<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetRequestKind::Socket(_) => write!(f, "NetRequestKind::Socket"),
            NetRequestKind::Grpc(grpc) => match grpc {
                NetRequestGrpc::Unary(_) => write!(f, "NetRequestKind::Grpc::Unary"),
                NetRequestGrpc::Stream(_) => write!(f, "NetRequestKind::Grpc::Stream"),
                NetRequestGrpc::Unsubscribe(_) => {
                    write!(f, "NetRequestKind::Grpc::Unsubscribe")
                }
            },
            NetRequestKind::Http(http) => {
                write!(f, "NetRequestKind::Http {{ url: {} }}", http.url)
            }
            NetRequestKind::InitTor(_) => write!(f, "NetRequestKind::InitTor"),
            NetRequestKind::TorInited => write!(f, "NetRequestKind::TorInited"),
        }
    }
}
impl<'a> fmt::Debug for NetRequest<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NetRequest {{ id: {}, transport_id: {}, kind: {:?} }}",
            self.id, self.transport_id, self.kind
        )
    }
}
