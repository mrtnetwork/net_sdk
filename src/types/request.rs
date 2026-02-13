use crate::{
    types::{
        config::{NetHttpHeaderC, NetProtocol},
        error::NetResultStatus,
    },
    utils::{Utils, buffer::StreamEncoding},
};
use libc::c_char;
use std::{mem::ManuallyDrop, slice};

pub struct NetGrpcRequestUnary<'a> {
    pub method: &'a str,
    pub data: &'a [u8],
}

pub struct NetGrpcRequestStream<'a> {
    pub method: &'a str,
    pub data: &'a [u8],
}

pub struct NetGrpcRequestUnsubscribe {
    pub id: i32,
}
pub struct NetHttpHeaderRef<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

pub enum GrpcRequest<'a> {
    Unary(NetGrpcRequestUnary<'a>),
    Stream(NetGrpcRequestStream<'a>),
    Unsubscribe(NetGrpcRequestUnsubscribe),
}

pub struct NetHttpRequest<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub body: Option<&'a [u8]>,
    pub headers: Option<Vec<NetHttpHeaderRef<'a>>>,
    pub encoding: StreamEncoding,
}

pub struct NetSocketRequestSend<'a> {
    pub data: &'a [u8],
}

pub enum NetSocketRequest<'a> {
    Subscribe,
    Unsubscribe,
    Send(NetSocketRequestSend<'a>),
}

pub enum NetRequestKind<'a> {
    Socket(NetSocketRequest<'a>),
    Grpc(GrpcRequest<'a>),
    Http(NetHttpRequest<'a>),
}

pub struct NetRequest<'a> {
    pub transport_id: u32,
    pub id: u32,
    pub timeout: u32,
    pub kind: NetRequestKind<'a>,
}

impl<'a> NetRequest<'a> {
    pub fn to_http_request(&'a self) -> Result<&'a NetHttpRequest<'a>, NetResultStatus> {
        match &self.kind {
            NetRequestKind::Http(http_request) => Ok(http_request),
            _ => Err(NetResultStatus::InvalidRequestParameters),
        }
    }
    pub fn to_socket_request(&'a self) -> Result<&'a NetSocketRequest<'a>, NetResultStatus> {
        match &self.kind {
            NetRequestKind::Socket(socket_request) => Ok(socket_request),
            _ => Err(NetResultStatus::InvalidRequestParameters),
        }
    }

    pub fn to_grpc_request(&'a self) -> Result<&'a GrpcRequest<'a>, NetResultStatus> {
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

#[repr(C)]
pub struct BytesRefC {
    pub ptr: *const u8,
    pub len: u32,
}
#[repr(C)]
pub struct NetGrpcRequestUnaryC {
    pub method: *const c_char,
    pub data: BytesRefC,
}
#[repr(C)]
pub struct NetGrpcRequestStreamC {
    pub method: *const c_char,
    pub data: BytesRefC,
}
#[repr(C)]
pub struct NetGrpcRequestUnsubscribeC {
    pub id: i32,
}
#[repr(C)]
pub struct NetHttpRequestC {
    pub method: *const c_char,
    pub url: *const c_char,
    pub body: BytesRefC,
    pub headers: *const NetHttpHeaderC,
    pub headers_len: u8,
    pub encoding: u8,
}
#[repr(C)]
pub struct NetSocketRequestSendC {
    pub data: BytesRefC,
}

#[repr(C)]
pub union NetGrpcRequestUnionC {
    pub unary: ManuallyDrop<*const NetGrpcRequestUnaryC>,
    pub stream: ManuallyDrop<*const NetGrpcRequestStreamC>,
    pub unsubscribe: ManuallyDrop<*const NetGrpcRequestUnsubscribeC>,
}

#[repr(C)]
pub union NetSocketRequestUnionC {
    pub send: ManuallyDrop<*const NetSocketRequestSendC>,
}
#[repr(C)]
pub struct NetGrpcRequestC {
    pub tag: u8,
    pub payload: NetGrpcRequestUnionC,
}

#[repr(C)]
pub struct NetSocketRequestC {
    pub tag: u8,
    pub payload: NetSocketRequestUnionC,
}
#[repr(C)]
pub union NetRequestKindUnionC {
    pub socket: ManuallyDrop<*const NetSocketRequestC>,
    pub grpc: ManuallyDrop<*const NetGrpcRequestC>,
    pub http: ManuallyDrop<*const NetHttpRequestC>,
}
#[repr(C)]
pub struct NetRequestKindC {
    pub tag: u8,
    pub payload: NetRequestKindUnionC,
}
#[repr(C)]
pub struct NetRequestC {
    pub transport_id: u32,
    pub id: u32,
    pub timeout: u32,
    pub kind: NetRequestKindC,
}

impl BytesRefC {
    pub unsafe fn free_memory(&self) {
        if self.ptr.is_null() || self.len == 0 {
            return;
        }

        // Reconstruct the Vec<u8> so Rust can drop it
        let _ = unsafe {
            Vec::from_raw_parts(self.ptr as *mut u8, self.len as usize, self.len as usize)
        };
    }
}

unsafe fn bytes_from_ref<'a>(b: &BytesRefC) -> &'a [u8] {
    unsafe { slice::from_raw_parts(b.ptr, b.len as usize) }
}
impl<'a> NetRequest<'a> {
    pub unsafe fn from_c(c: &NetRequestC) -> Result<Self, NetResultStatus> {
        Ok(NetRequest {
            transport_id: c.transport_id,
            id: c.id,
            timeout: c.timeout,
            kind: match c.kind.tag {
                1 => {
                    let pointer = unsafe { c.kind.payload.socket.as_ref() };
                    match pointer {
                        Some(u) => NetRequestKind::Socket(unsafe { NetSocketRequest::from_c(u) }?),
                        None => return Err(NetResultStatus::InvalidRequestParameters),
                    }
                }
                2 => {
                    let pointer = unsafe { c.kind.payload.grpc.as_ref() };
                    match pointer {
                        Some(u) => NetRequestKind::Grpc(unsafe { GrpcRequest::from_c(u) }?),
                        None => return Err(NetResultStatus::InvalidRequestParameters),
                    }
                }
                3 => {
                    let pointer = unsafe { c.kind.payload.http.as_ref() };
                    match pointer {
                        Some(u) => NetRequestKind::Http(unsafe { NetHttpRequest::from_c(u) }?),
                        None => return Err(NetResultStatus::InvalidRequestParameters),
                    }
                }
                _ => return Err(NetResultStatus::InvalidRequestParameters),
            },
        })
    }
}
impl<'a> GrpcRequest<'a> {
    pub unsafe fn from_c(c: &NetGrpcRequestC) -> Result<Self, NetResultStatus> {
        Ok(match c.tag {
            1 => {
                let pointer = unsafe { c.payload.unary.as_ref() };
                match pointer {
                    Some(u) => {
                        if u.method.is_null() {
                            return Err(NetResultStatus::InvalidRequestParameters);
                        }
                        GrpcRequest::Unary(NetGrpcRequestUnary {
                            method: unsafe { Utils::cstr_to_str(u.method as *const u8) },
                            data: unsafe { bytes_from_ref(&u.data) },
                        })
                    }
                    None => return Err(NetResultStatus::InvalidRequestParameters),
                }
            }
            2 => {
                let pointer = unsafe { c.payload.stream.as_ref() };
                match pointer {
                    Some(s) => {
                        if s.method.is_null() {
                            return Err(NetResultStatus::InvalidRequestParameters);
                        }
                        GrpcRequest::Stream(NetGrpcRequestStream {
                            method: unsafe { Utils::cstr_to_str(s.method as *const u8) },
                            data: unsafe { bytes_from_ref(&s.data) },
                        })
                    }
                    None => return Err(NetResultStatus::InvalidRequestParameters),
                }
            }
            3 => {
                let pointer = unsafe { c.payload.unsubscribe.as_ref() };
                match pointer {
                    Some(u) => GrpcRequest::Unsubscribe(NetGrpcRequestUnsubscribe { id: u.id }),
                    None => return Err(NetResultStatus::InvalidRequestParameters),
                }
            }

            _ => return Err(NetResultStatus::InvalidRequestParameters),
        })
    }
}
impl<'a> NetSocketRequest<'a> {
    pub unsafe fn from_c(c: &NetSocketRequestC) -> Result<Self, NetResultStatus> {
        Ok(match c.tag {
            1 => {
                let pointer = unsafe { c.payload.send.as_ref() };
                match pointer {
                    Some(u) => NetSocketRequest::Send(NetSocketRequestSend {
                        data: unsafe { bytes_from_ref(&u.data) },
                    }),
                    None => return Err(NetResultStatus::InvalidRequestParameters),
                }
            }
            2 => NetSocketRequest::Subscribe,
            3 => NetSocketRequest::Unsubscribe,
            _ => return Err(NetResultStatus::InvalidRequestParameters),
        })
    }
}
impl<'a> NetHttpHeaderRef<'a> {
    fn from_c(c: &NetHttpHeaderC) -> Result<Self, NetResultStatus> {
        if c.key.is_null() || c.value.is_null() {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        Ok(Self {
            key: unsafe { Utils::cstr_to_str(c.key as *const u8) },
            value: unsafe { Utils::cstr_to_str(c.value as *const u8) },
        })
    }
}

impl<'a> NetHttpRequest<'a> {
    pub unsafe fn from_c(c: &NetHttpRequestC) -> Result<Self, NetResultStatus> {
        if c.method.is_null() || c.url.is_null() {
            return Err(NetResultStatus::InvalidRequestParameters);
        }
        let headers = if c.headers.is_null() {
            if c.headers_len != 0 {
                return Err(NetResultStatus::InvalidRequestParameters);
            }
            None
        } else {
            Some(unsafe {
                std::slice::from_raw_parts(c.headers, c.headers_len.into())
                    .iter()
                    .map(NetHttpHeaderRef::from_c) // don't use `?` here
                    .collect::<Result<Vec<_>, _>>()?
            })
        };
        Ok(NetHttpRequest {
            method: unsafe { Utils::cstr_to_str(c.method as *const u8) },
            url: unsafe { Utils::cstr_to_str(c.url as *const u8) },
            body: match c.body.len {
                0 => None,
                _ => Some(unsafe { bytes_from_ref(&c.body) }),
            },
            encoding: match c.encoding {
                1 => StreamEncoding::Json,
                2 => StreamEncoding::Raw,
                3 => StreamEncoding::CborJson,
                _ => return Err(NetResultStatus::InvalidRequestParameters),
            },
            headers: headers,
        })
    }
}
