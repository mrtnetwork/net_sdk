use std::mem::ManuallyDrop;

use crate::{
    types::{
        config::{NetHttpHeader, NetHttpHeaderC},
        error::NetResultStatus,
        request::BytesRefC,
    },
    utils::{Utils, buffer::StreamEncoding},
};
pub struct NetSocketResponseOk;
pub struct NetStreamResponseData {
    pub id: Option<i32>,
    pub data: Vec<u8>,
}
pub struct NetStreamResponseError {
    pub id: Option<i32>,
    pub status: NetResultStatus,
}

pub enum NetStreamResponse {
    Data(NetStreamResponseData),
    Close(Option<i32>),
    Error(NetStreamResponseError),
}
#[derive(Clone, Debug)]
pub struct NetHttpResponse {
    pub status_code: u16,
    pub body: Vec<u8>,
    pub headers: Vec<NetHttpHeader>,
    pub encoding: StreamEncoding,
}

pub struct NetGrpcStreamIdResponse {
    pub id: i32,
}

pub struct NetGrpcUnsubscribeStreamIdResponse {
    pub id: i32,
}

pub struct NetGrpcUnaryResponse {
    pub data: Vec<u8>,
}
pub enum NetGrpcResponse {
    Unary(NetGrpcUnaryResponse),
    StreamId(NetGrpcStreamIdResponse),
    Unsubscribe(NetGrpcUnsubscribeStreamIdResponse),
}

pub struct NetResponse {
    pub transport_id: u32,
    pub request_id: u32,
    pub response: NetResponseKind,
}
pub enum NetResponseKind {
    Socket(NetSocketResponseOk),
    Grpc(NetGrpcResponse),
    Http(NetHttpResponse),
    Stream(NetStreamResponse),
    ResponseError(NetResultStatus),
    TransportClosed,
}

#[repr(C)]
pub struct NetStreamResponseDataC {
    pub id: i32,
    pub data: BytesRefC,
}
#[repr(C)]
pub struct NetStreamResponseCloseC {
    pub id: i32,
}

#[repr(C)]
pub struct NetStreamResponseErrorC {
    pub id: i32,
    pub error: u8,
}
#[repr(C)]
pub union NetStreamResponseUnionC {
    pub data: ManuallyDrop<NetStreamResponseDataC>,
    pub close: ManuallyDrop<NetStreamResponseCloseC>,
    pub error: ManuallyDrop<NetStreamResponseErrorC>,
}
#[repr(C)]
pub struct NetStreamResponseC {
    pub tag: u8,
    pub payload: NetStreamResponseUnionC,
}
#[repr(C)]
pub struct NetSocketStreamResponseOkC;
#[repr(C)]
pub struct NetHttpResponseC {
    pub status_code: u16,
    pub body: BytesRefC,
    pub headers: *const NetHttpHeaderC,
    pub headers_len: u32,
    pub encoding: u8,
}
#[repr(C)]
pub struct NetGrpcUnaryResponseC {
    pub data: BytesRefC,
}
#[repr(C)]
pub struct NetGrpcStreamIdResponseC {
    pub id: i32,
}

#[repr(C)]
pub struct NetGrpcUnsubscribeStreamIdResponseC {
    pub id: i32,
}

#[repr(C)]
pub union NetGrpcResponseUnionC {
    pub unary: ManuallyDrop<NetGrpcUnaryResponseC>,
    pub stream_id: ManuallyDrop<NetGrpcStreamIdResponseC>,
    pub unsubscribe: ManuallyDrop<NetGrpcUnsubscribeStreamIdResponseC>,
}

#[repr(C)]
pub struct NetGrpcResponseC {
    pub tag: u8,
    pub payload: NetGrpcResponseUnionC,
}
#[repr(C)]
pub struct NetResponseTransportClosedC;
#[repr(C)]
pub struct NetResponseErrorC {
    pub error: u8,
}
#[repr(C)]
pub union NetResponseKindUnionC {
    pub socket: ManuallyDrop<NetSocketStreamResponseOkC>,
    pub grpc: ManuallyDrop<NetGrpcResponseC>,
    pub http: ManuallyDrop<NetHttpResponseC>,
    pub stream: ManuallyDrop<NetStreamResponseC>,
    pub error: ManuallyDrop<NetResponseErrorC>,
    pub closed: ManuallyDrop<NetResponseTransportClosedC>,
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
fn bytes_to_ref(bytes: &Vec<u8>) -> BytesRefC {
    let boxed = bytes.clone().into_boxed_slice();
    let ptr = boxed.as_ptr();
    let len = boxed.len();
    std::mem::forget(boxed);
    BytesRefC {
        ptr: ptr,
        len: len as u32,
    }
}
impl NetGrpcResponse {
    pub fn to_c(&self) -> NetGrpcResponseC {
        match self {
            NetGrpcResponse::Unary(u) => NetGrpcResponseC {
                tag: 1,
                payload: NetGrpcResponseUnionC {
                    unary: ManuallyDrop::new(NetGrpcUnaryResponseC {
                        data: bytes_to_ref(&u.data),
                    }),
                },
            },

            NetGrpcResponse::StreamId(s) => NetGrpcResponseC {
                tag: 2,
                payload: NetGrpcResponseUnionC {
                    stream_id: ManuallyDrop::new(NetGrpcStreamIdResponseC { id: s.id }),
                },
            },

            NetGrpcResponse::Unsubscribe(s) => NetGrpcResponseC {
                tag: 3,
                payload: NetGrpcResponseUnionC {
                    unsubscribe: ManuallyDrop::new(NetGrpcUnsubscribeStreamIdResponseC {
                        id: s.id,
                    }),
                },
            },
        }
    }
}

impl NetStreamResponse {
    pub fn to_c(&self) -> NetStreamResponseC {
        match self {
            NetStreamResponse::Data(u) => NetStreamResponseC {
                tag: 1,
                payload: NetStreamResponseUnionC {
                    data: ManuallyDrop::new(NetStreamResponseDataC {
                        data: bytes_to_ref(&u.data),
                        id: u.id.map_or(-1, |e| e),
                    }),
                },
            },

            NetStreamResponse::Close(n) => NetStreamResponseC {
                tag: 2,
                payload: NetStreamResponseUnionC {
                    close: ManuallyDrop::new(NetStreamResponseCloseC {
                        id: n.map_or(-1, |e| e),
                    }),
                },
            },
            NetStreamResponse::Error(e) => NetStreamResponseC {
                tag: 3,
                payload: NetStreamResponseUnionC {
                    error: ManuallyDrop::new(NetStreamResponseErrorC {
                        error: e.status as u8,
                        id: e.id.map_or(-1, |e| e),
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
                let (headers_ptr, headers_len) = NetHttpHeader::headers_to_c(&h.headers);
                NetResponseKindC {
                    tag: 3,
                    payload: NetResponseKindUnionC {
                        http: ManuallyDrop::new(NetHttpResponseC {
                            status_code: h.status_code,
                            body: bytes_to_ref(&h.body),
                            headers: headers_ptr,
                            headers_len,
                            encoding: h.encoding as u8,
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
                        debug_assert!(false, "Unknown NetGrpcResponseC tag");
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
impl NetHttpHeader {
    pub fn headers_to_c(headers: &[NetHttpHeader]) -> (*const NetHttpHeaderC, u32) {
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

                (*slot).key = Utils::string_to_c_ptr(&h.key) as *const _;
                (*slot).value = Utils::string_to_c_ptr(&h.value) as *const _;
            }
        }

        (ptr, len as u32)
    }
}

impl NetHttpResponseC {
    pub unsafe fn free_memory(&self) {
        unsafe { self.body.free_memory() };
        if self.headers.is_null() {
            return;
        }

        for i in 0..self.headers_len as usize {
            let h = unsafe { self.headers.add(i) };

            unsafe { Utils::free_c_string((*h).key as *mut u8) };
            unsafe { Utils::free_c_string((*h).value as *mut u8) };
        }

        unsafe { libc::free(self.headers as *mut libc::c_void) };
    }
}
