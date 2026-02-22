use std::{mem::ManuallyDrop, slice};

use crate::{
    types::{
        config::{
            NetConfigHttp, NetConfigRequest, NetConfigTor, NetHttpHeader, NetHttpProtocol, NetMode,
            NetProtocol, NetTlsMode,
        },
        error::NetResultStatus,
        native::request::{
            NetHttpHeaderRef, NetHttpRetryConfig, NetRequest, NetRequestGrpc, NetRequestGrpcStream,
            NetRequestGrpcUnary, NetRequestGrpcUnsubscribe, NetRequestHttp, NetRequestKind,
            NetRequestSocket, NetRequestSocketSend,
        },
        response::{NetResponse, NetResponseGrpc, NetResponseKind, NetResponseStream},
    },
    utils::{Utils, buffer::StreamEncoding},
};
use libc::c_char;

/// configs
#[repr(C)]
pub struct NetHttpHeaderC {
    pub key: *const c_char,
    pub value: *const c_char,
}

#[repr(C)]
pub struct NetConfigHttpC {
    pub headers: *const NetHttpHeaderC,
    pub headers_len: u8,

    pub protocol: u8,
}

#[repr(C)]
pub struct NetConfigTorC {
    pub cache_dir: *const c_char,
    pub state_dir: *const c_char,
}

#[repr(C)]
pub struct NetConfigRequestC {
    pub url: *const c_char,
    pub mode: u8,
    pub protocol: u8,
    pub http: *const NetConfigHttpC,
    pub tls_mode: u8,
    pub stream_encoding: u8,
}
impl TryFrom<&NetHttpHeaderC> for NetHttpHeader {
    type Error = NetResultStatus;
    fn try_from(c: &NetHttpHeaderC) -> Result<Self, NetResultStatus> {
        if c.key.is_null() || c.value.is_null() {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        Ok(Self::new(
            unsafe { Utils::cstr_to_string(c.key as *const u8) },
            unsafe { Utils::cstr_to_string(c.value as *const u8) },
        ))
    }
}

impl TryFrom<&NetConfigTorC> for NetConfigTor {
    type Error = NetResultStatus;
    fn try_from(c: &NetConfigTorC) -> Result<Self, NetResultStatus> {
        if c.cache_dir.is_null() || c.state_dir.is_null() {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        Ok(Self {
            cache_dir: unsafe { Utils::cstr_to_string(c.cache_dir as *const u8) },
            state_dir: unsafe { Utils::cstr_to_string(c.state_dir as *const u8) },
        })
    }
}
impl TryFrom<&NetConfigHttpC> for NetConfigHttp {
    type Error = NetResultStatus;
    fn try_from(c: &NetConfigHttpC) -> Result<Self, NetResultStatus> {
        let headers = if c.headers.is_null() {
            Vec::new()
        } else {
            unsafe {
                std::slice::from_raw_parts(c.headers, c.headers_len.into())
                    .iter()
                    .map(NetHttpHeader::try_from) // don't use `?` here
                    .collect::<Result<Vec<_>, _>>()? //
            }
        };

        let protocol = match c.protocol {
            0 => None,
            1 => Some(NetHttpProtocol::Http1),
            2 => Some(NetHttpProtocol::Http2),
            _ => return Err(NetResultStatus::InvalidConfigParameters),
        };
        Ok(Self { headers, protocol })
    }
}
impl TryFrom<&NetConfigRequestC> for NetConfigRequest {
    type Error = NetResultStatus;
    fn try_from(c: &NetConfigRequestC) -> Result<Self, NetResultStatus> {
        let http = unsafe {
            c.http
                .as_ref()
                .map(NetConfigHttp::try_from)
                .transpose()?
                .ok_or(NetResultStatus::InvalidConfigParameters)?
        };
        if c.url.is_null() {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        Ok(Self {
            url: unsafe { Utils::cstr_to_string(c.url as *const u8) },
            mode: match c.mode {
                1 => NetMode::Tor,
                2 => NetMode::Clearnet,
                _ => return Err(NetResultStatus::InvalidConfigParameters),
            },
            protocol: match c.protocol {
                1 => NetProtocol::Http,
                2 => NetProtocol::Grpc,
                3 => NetProtocol::WebSocket,
                4 => NetProtocol::Socket,
                _ => return Err(NetResultStatus::InvalidConfigParameters),
            },
            tls_mode: match c.tls_mode {
                1 => NetTlsMode::Safe,
                2 => NetTlsMode::Dangerous,
                _ => return Err(NetResultStatus::InvalidConfigParameters),
            },
            encoding: match c.stream_encoding {
                1 => StreamEncoding::Json,
                2 => StreamEncoding::Raw,
                _ => return Err(NetResultStatus::InvalidConfigParameters),
            },
            http,
        })
    }
}

/// request

#[repr(C)]
pub struct BytesRefC {
    pub ptr: *const u8,
    pub len: u32,
}
#[repr(C)]
pub struct NetRequestGrpcUnaryC {
    pub method: *const c_char,
    pub data: BytesRefC,
}
#[repr(C)]
pub struct NetRequestGrpcStreamC {
    pub method: *const c_char,
    pub data: BytesRefC,
}
#[repr(C)]
pub struct NetRequestGrpcUnsubscribeC {
    pub id: i32,
}
#[repr(C)]
pub struct NetRequestHttpC {
    pub method: *const c_char,
    pub url: *const c_char,
    pub body: BytesRefC,
    pub headers: *const NetHttpHeaderC,
    pub headers_len: u8,
    pub encoding: u8,
    pub retry_config: *const NetHttpRetryConfigC,
}

pub struct NetHttpRetryConfigC {
    pub retry_status: *const u16,
    pub len: u8,
    pub max_retries: u8,
    pub retry_delay: u32,
}
#[repr(C)]
pub struct NetRequestSocketSendC {
    pub data: BytesRefC,
}

#[repr(C)]
pub union NetRequestGrpcUnionC {
    pub unary: ManuallyDrop<*const NetRequestGrpcUnaryC>,
    pub stream: ManuallyDrop<*const NetRequestGrpcStreamC>,
    pub unsubscribe: ManuallyDrop<*const NetRequestGrpcUnsubscribeC>,
}

#[repr(C)]
pub union NetRequestSocketUnionC {
    pub send: ManuallyDrop<*const NetRequestSocketSendC>,
}
#[repr(C)]
pub struct NetRequestGrpcC {
    pub tag: u8,
    pub payload: NetRequestGrpcUnionC,
}

#[repr(C)]
pub struct NetRequestSocketC {
    pub tag: u8,
    pub payload: NetRequestSocketUnionC,
}
#[repr(C)]
pub union NetRequestKindUnionC {
    pub socket: ManuallyDrop<*const NetRequestSocketC>,
    pub grpc: ManuallyDrop<*const NetRequestGrpcC>,
    pub http: ManuallyDrop<*const NetRequestHttpC>,
    pub init_tor: ManuallyDrop<*const NetConfigTorC>,
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
unsafe fn u16_from_ref<'a>(v: *const u16, len: u8) -> &'a [u16] {
    unsafe { slice::from_raw_parts(v, len as usize) }
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
                        Some(u) => NetRequestKind::Socket(unsafe { NetRequestSocket::from_c(u) }?),
                        None => return Err(NetResultStatus::InvalidRequestParameters),
                    }
                }
                2 => {
                    let pointer = unsafe { c.kind.payload.grpc.as_ref() };
                    match pointer {
                        Some(u) => NetRequestKind::Grpc(unsafe { NetRequestGrpc::from_c(u) }?),
                        None => return Err(NetResultStatus::InvalidRequestParameters),
                    }
                }
                3 => {
                    let pointer = unsafe { c.kind.payload.http.as_ref() };
                    match pointer {
                        Some(u) => NetRequestKind::Http(unsafe { NetRequestHttp::from_c(u) }?),
                        None => return Err(NetResultStatus::InvalidRequestParameters),
                    }
                }
                4 => {
                    let pointer = unsafe { c.kind.payload.init_tor.as_ref() };
                    match pointer {
                        Some(u) => NetRequestKind::InitTor(NetConfigTor::try_from(u)?),
                        None => return Err(NetResultStatus::InvalidRequestParameters),
                    }
                }
                5 => NetRequestKind::TorInited,
                _ => return Err(NetResultStatus::InvalidRequestParameters),
            },
        })
    }
}
impl<'a> NetRequestGrpc<'a> {
    pub unsafe fn from_c(c: &NetRequestGrpcC) -> Result<Self, NetResultStatus> {
        Ok(match c.tag {
            1 => {
                let pointer = unsafe { c.payload.unary.as_ref() };
                match pointer {
                    Some(u) => {
                        if u.method.is_null() {
                            return Err(NetResultStatus::InvalidRequestParameters);
                        }
                        NetRequestGrpc::Unary(NetRequestGrpcUnary {
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
                        NetRequestGrpc::Stream(NetRequestGrpcStream {
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
                    Some(u) => NetRequestGrpc::Unsubscribe(NetRequestGrpcUnsubscribe { id: u.id }),
                    None => return Err(NetResultStatus::InvalidRequestParameters),
                }
            }

            _ => return Err(NetResultStatus::InvalidRequestParameters),
        })
    }
}
impl<'a> NetRequestSocket<'a> {
    pub unsafe fn from_c(c: &NetRequestSocketC) -> Result<Self, NetResultStatus> {
        Ok(match c.tag {
            1 => {
                let pointer = unsafe { c.payload.send.as_ref() };
                match pointer {
                    Some(u) => NetRequestSocket::Send(NetRequestSocketSend {
                        data: unsafe { bytes_from_ref(&u.data) },
                    }),
                    None => return Err(NetResultStatus::InvalidRequestParameters),
                }
            }
            2 => NetRequestSocket::Subscribe,
            3 => NetRequestSocket::Unsubscribe,
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
impl<'a> NetHttpRetryConfig<'a> {
    pub unsafe fn from_c(c: &NetHttpRetryConfigC) -> Result<Self, NetResultStatus> {
        return Ok(unsafe {
            NetHttpRetryConfig {
                retry_status: u16_from_ref(c.retry_status, c.len),
                max_retries: c.max_retries,
                retry_delay: c.retry_delay,
            }
        });
    }
}
impl<'a> NetRequestHttp<'a> {
    pub unsafe fn from_c(c: &NetRequestHttpC) -> Result<Self, NetResultStatus> {
        if c.method.is_null() || c.url.is_null() {
            return Err(NetResultStatus::InvalidRequestParameters);
        }
        let headers: Option<Vec<super::request::NetHttpHeaderRef<'_>>> = if c.headers.is_null() {
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
        let pointer = unsafe { c.retry_config.as_ref() };
        let retry = pointer.map_or(Ok(NetHttpRetryConfig::default()), |e| unsafe {
            NetHttpRetryConfig::from_c(e)
        })?;
        Ok(NetRequestHttp {
            method: unsafe { Utils::cstr_to_str(c.method as *const u8) },
            url: unsafe { Utils::cstr_to_str(c.url as *const u8) },
            retry_config: retry,
            body: match c.body.len {
                0 => None,
                _ => Some(unsafe { bytes_from_ref(&c.body) }),
            },
            encoding: match c.encoding {
                1 => StreamEncoding::Json,
                2 => StreamEncoding::Raw,
                _ => return Err(NetResultStatus::InvalidRequestParameters),
            },
            headers: headers,
        })
    }
}
/// response

#[repr(C)]
pub struct NetResponseStreamDataC {
    pub id: i32,
    pub data: BytesRefC,
}
#[repr(C)]
pub struct NetResponseStreamCloseC {
    pub id: i32,
}

#[repr(C)]
pub struct NetResponseStreamErrorC {
    pub id: i32,
    pub error: u8,
}
#[repr(C)]
pub union NetResponseStreamUnionC {
    pub data: ManuallyDrop<NetResponseStreamDataC>,
    pub close: ManuallyDrop<NetResponseStreamCloseC>,
    pub error: ManuallyDrop<NetResponseStreamErrorC>,
}
#[repr(C)]
pub struct NetResponseStreamC {
    pub tag: u8,
    pub payload: NetResponseStreamUnionC,
}
#[repr(C)]
pub struct NetSocketStreamResponseOkC;
#[repr(C)]
pub struct NetResponseHttpC {
    pub status_code: u16,
    pub body: BytesRefC,
    pub headers: *const NetHttpHeaderC,
    pub headers_len: u32,
}
#[repr(C)]
pub struct NetResponseGrpcUnaryC {
    pub data: BytesRefC,
}
#[repr(C)]
pub struct NetResponseGrpcSubscribeC {
    pub id: i32,
}

#[repr(C)]
pub struct NetResponseGrpcUnsubscribeC {
    pub id: i32,
}

#[repr(C)]
pub union NetResponseGrpcUnionC {
    pub unary: ManuallyDrop<NetResponseGrpcUnaryC>,
    pub stream_id: ManuallyDrop<NetResponseGrpcSubscribeC>,
    pub unsubscribe: ManuallyDrop<NetResponseGrpcUnsubscribeC>,
}

#[repr(C)]
pub struct NetResponseGrpcC {
    pub tag: u8,
    pub payload: NetResponseGrpcUnionC,
}
#[repr(C)]
pub struct NetResponseTransportClosedC;
#[repr(C)]
pub struct NetResponseTorInited {
    pub inited: bool,
}
#[repr(C)]
pub struct NetResponseErrorC {
    pub error: u8,
}
#[repr(C)]
pub union NetResponseKindUnionC {
    pub socket: ManuallyDrop<NetSocketStreamResponseOkC>,
    pub grpc: ManuallyDrop<NetResponseGrpcC>,
    pub http: ManuallyDrop<NetResponseHttpC>,
    pub stream: ManuallyDrop<NetResponseStreamC>,
    pub error: ManuallyDrop<NetResponseErrorC>,
    pub closed: ManuallyDrop<NetResponseTransportClosedC>,
    pub tor_inited: ManuallyDrop<NetResponseTorInited>,
}

#[repr(C)]
pub struct NetResponseKindC {
    pub tag: u8,
    pub payload: NetResponseKindUnionC,
}
#[repr(C)]
pub struct NetResponseC {
    pub transport_id: u32,
    pub request_id: u32,
    pub response: NetResponseKindC,
}

#[inline]
fn bytes_to_ref(bytes: Vec<u8>) -> BytesRefC {
    let boxed = bytes.clone().into_boxed_slice();
    let ptr = boxed.as_ptr();
    let len = boxed.len();
    std::mem::forget(boxed);
    BytesRefC {
        ptr: ptr,
        len: len as u32,
    }
}
impl NetResponseGrpc {
    pub fn to_c(&self) -> NetResponseGrpcC {
        match self {
            NetResponseGrpc::Unary(u) => NetResponseGrpcC {
                tag: 1,
                payload: NetResponseGrpcUnionC {
                    unary: ManuallyDrop::new(NetResponseGrpcUnaryC {
                        data: bytes_to_ref(u.data()),
                    }),
                },
            },

            NetResponseGrpc::StreamId(s) => NetResponseGrpcC {
                tag: 2,
                payload: NetResponseGrpcUnionC {
                    stream_id: ManuallyDrop::new(NetResponseGrpcSubscribeC { id: s.id() }),
                },
            },

            NetResponseGrpc::Unsubscribe(s) => NetResponseGrpcC {
                tag: 3,
                payload: NetResponseGrpcUnionC {
                    unsubscribe: ManuallyDrop::new(NetResponseGrpcUnsubscribeC { id: s.id() }),
                },
            },
        }
    }
}

impl NetResponseStream {
    pub fn to_c(&self) -> NetResponseStreamC {
        match self {
            NetResponseStream::Data(u) => NetResponseStreamC {
                tag: 1,
                payload: NetResponseStreamUnionC {
                    data: ManuallyDrop::new(NetResponseStreamDataC {
                        data: bytes_to_ref(u.data()),
                        id: u.id().map_or(-1, |e| e),
                    }),
                },
            },

            NetResponseStream::Close(n) => NetResponseStreamC {
                tag: 2,
                payload: NetResponseStreamUnionC {
                    close: ManuallyDrop::new(NetResponseStreamCloseC {
                        id: n.map_or(-1, |e| e),
                    }),
                },
            },
            NetResponseStream::Error(e) => NetResponseStreamC {
                tag: 3,
                payload: NetResponseStreamUnionC {
                    error: ManuallyDrop::new(NetResponseStreamErrorC {
                        error: e.status() as u8,
                        id: e.id().map_or(-1, |e| e),
                    }),
                },
            },
        }
    }
}
impl NetResponse {
    pub fn to_c(&self) -> NetResponseC {
        NetResponseC {
            transport_id: self.transport_id,
            request_id: self.request_id,
            response: self.response.to_c(),
        }
    }
}
impl NetResponseKind {
    pub fn to_c(&self) -> NetResponseKindC {
        match self {
            NetResponseKind::Socket(_) => NetResponseKindC {
                tag: 1,
                payload: NetResponseKindUnionC {
                    socket: ManuallyDrop::new(NetSocketStreamResponseOkC),
                },
            },
            NetResponseKind::Grpc(g) => NetResponseKindC {
                tag: 2,
                payload: NetResponseKindUnionC {
                    grpc: ManuallyDrop::new(g.to_c()),
                },
            },
            NetResponseKind::Http(h) => {
                let (headers_ptr, headers_len) = NetHttpHeader::headers_to_c(h.headers());
                NetResponseKindC {
                    tag: 3,
                    payload: NetResponseKindUnionC {
                        http: ManuallyDrop::new(NetResponseHttpC {
                            status_code: h.status_code(),
                            body: bytes_to_ref(h.body()),
                            headers: headers_ptr,
                            headers_len,
                        }),
                    },
                }
            }
            NetResponseKind::Stream(net_stream_response) => NetResponseKindC {
                tag: 4,
                payload: NetResponseKindUnionC {
                    stream: ManuallyDrop::new(net_stream_response.to_c()),
                },
            },
            NetResponseKind::ResponseError(net_result_status) => NetResponseKindC {
                tag: 5,
                payload: NetResponseKindUnionC {
                    error: ManuallyDrop::new(NetResponseErrorC {
                        error: *net_result_status as u8,
                    }),
                },
            },
            NetResponseKind::TransportClosed => NetResponseKindC {
                tag: 6,
                payload: NetResponseKindUnionC {
                    closed: ManuallyDrop::new(NetResponseTransportClosedC),
                },
            },
            NetResponseKind::TorInited(inited) => NetResponseKindC {
                tag: 7,
                payload: NetResponseKindUnionC {
                    tor_inited: ManuallyDrop::new(NetResponseTorInited { inited: *inited }),
                },
            },
        }
    }
}

impl NetResponseC {
    pub unsafe fn free_memory(&self) {
        match self.response.tag {
            2 => {
                let grpc = unsafe { &self.response.payload.grpc };

                match grpc.tag {
                    1 => {
                        unsafe { grpc.payload.unary.data.free_memory() };
                    }
                    2 | 3 => {}
                    _ => {
                        debug_assert!(false, "Unknown NetResponseGrpcC tag");
                    }
                }
            }
            3 => {
                let http = unsafe { &self.response.payload.http };
                unsafe { http.free_memory() };
            }

            4 => {
                let stream = unsafe { &self.response.payload.stream };
                match stream.tag {
                    1 => {
                        unsafe { stream.payload.data.data.free_memory() };
                    }
                    2 | 3 => {}
                    _ => {
                        debug_assert!(false, "Unknown NetResponseKindC tag")
                    }
                }
            }
            1 | 5 | 6 => {}

            _ => {
                debug_assert!(false, "Unknown NetResponseKindC tag");
            }
        }
    }
}
unsafe fn string_to_c_ptr(s: String) -> *mut u8 {
    let len = s.len();

    let buf = unsafe { libc::malloc(len + 1) } as *mut u8;
    if buf.is_null() {
        return std::ptr::null_mut();
    }

    unsafe { std::ptr::copy_nonoverlapping(s.as_ptr(), buf, len) };
    unsafe { *buf.add(len) = 0 };

    buf
}

unsafe fn free_c_string(ptr: *mut u8) {
    if !ptr.is_null() {
        unsafe { libc::free(ptr as *mut libc::c_void) };
    }
}
impl NetHttpHeader {
    pub fn headers_to_c(headers: Vec<NetHttpHeader>) -> (*const NetHttpHeaderC, u32) {
        let len = headers.len();

        if len == 0 {
            return (std::ptr::null(), 0);
        }

        let size = std::mem::size_of::<NetHttpHeaderC>() * len;

        let ptr = unsafe { libc::malloc(size) as *mut NetHttpHeaderC };
        if ptr.is_null() {
            return (std::ptr::null(), 0);
        }

        for (i, h) in headers.iter().enumerate() {
            unsafe {
                let slot = ptr.add(i);

                (*slot).key = string_to_c_ptr(h.key()) as *const _;
                (*slot).value = string_to_c_ptr(h.value()) as *const _;
            }
        }

        (ptr, len as u32)
    }
}

impl NetResponseHttpC {
    pub unsafe fn free_memory(&self) {
        unsafe { self.body.free_memory() };
        if self.headers.is_null() {
            return;
        }

        for i in 0..self.headers_len as usize {
            let h = unsafe { self.headers.add(i) };

            unsafe { free_c_string((*h).key as *mut u8) };
            unsafe { free_c_string((*h).value as *mut u8) };
        }

        unsafe { libc::free(self.headers as *mut libc::c_void) };
    }
}
