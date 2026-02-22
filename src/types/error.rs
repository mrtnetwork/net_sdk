use std::fmt;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum NetResultStatus {
    OK = 100,
    InvalidUrl = 1,
    TlsError = 2,
    ConnectionError = 3,
    TorNetError = 4,
    SocketError = 10,

    Http2ConctionFailed = 13,
    InvalidRequestParameters = 15,
    InvalidConfigParameters = 16,
    TransportNotFound = 17,
    // NotInitialized = 19,
    RequestTimeout = 22,
    InvalidTorConfig = 23,
    TorInitializationFailed = 24,
    TorClientNotInitialized = 26,
    InternalError = 27,
    InstanceDoesNotExist = 28,
}

impl fmt::Display for NetResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for NetResultStatus {}
