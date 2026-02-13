use std::fmt;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum NetResultStatus {
    OK = 100,
    InvalidUrl = 1,
    TlsError = 2,
    NetError = 3,
    TorNetError = 4,
    SocketError = 10,

    Http2ConctionFailed = 13,
    InvalidRequestParameters = 15,
    InvalidConfigParameters = 16,
    TransportNotFound = 17,
    InvalidResponse = 18,
    NotInitialized = 19,
    AlreadyInitialized = 20,
    InitializationFailed = 21,
    RequestTimeout = 22,
    InvalidTorConfig = 23,
    TorConnectionFailed = 24,
    MismatchHttpUrl = 25,
}

impl fmt::Display for NetResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for NetResultStatus {}
