use libc::c_char;

use crate::{
    types::{AddressInfo, error::NetResultStatus},
    utils::{Utils, buffer::StreamEncoding},
};
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum NetMode {
    Tor = 1,
    Clearnet = 2,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum NetHttpProtocol {
    Http1 = 1,
    Http2 = 2,
}
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NetProtocol {
    Http = 1,
    Grpc = 2,
    WebSocket = 3,
    Socket = 4,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum TlsMode {
    Safe = 1,
    Dangerous = 2,
}

#[derive(Clone, Debug)]
pub struct NetHttpHeader {
    pub key: String,
    pub value: String,
}

// #[derive(Clone, Debug)]
// pub struct NetHttpDigestAuth {
//     pub key: String,
//     pub value: String,
// }
#[derive(Clone, Debug)]
pub struct NetHttpConfig {
    pub headers: Vec<NetHttpHeader>,
    pub protocol: Option<NetHttpProtocol>,
    pub global_client: bool,
}
#[derive(Clone, Debug)]
pub struct NetTorClientConfig {
    pub cache_dir: String,
    pub state_dir: String,
}
pub struct NetRequestConfig {
    pub url: String,
    pub mode: NetMode,
    pub protocol: NetProtocol,
    pub http: NetHttpConfig,
    pub tls_mode: TlsMode,
    pub tor_config: Option<NetTorClientConfig>,
    pub encoding: StreamEncoding,
}

#[derive(Clone, Debug)]
pub struct NetConfig {
    pub addr: AddressInfo,
    pub mode: NetMode,
    pub http: NetHttpConfig,
    pub protocol: NetProtocol,
    pub tls_mode: TlsMode,
    pub tor_config: Option<NetTorClientConfig>,
    pub encoding: StreamEncoding,
}
impl NetConfig {
    pub fn change_addr(&self, new_addr: AddressInfo) -> NetConfig {
        Self {
            addr: new_addr,
            mode: self.mode,
            http: self.http.clone(),
            protocol: self.protocol,
            tls_mode: self.tls_mode,
            tor_config: self.tor_config.clone(),
            encoding: self.encoding.clone(),
        }
    }
}

impl Default for NetHttpConfig {
    fn default() -> NetHttpConfig {
        Self {
            headers: Vec::new(),
            protocol: None,
            global_client: false,
        }
    }
}

impl NetRequestConfig {
    fn to_protocol_address(&self) -> Result<AddressInfo, NetResultStatus> {
        match self.protocol {
            NetProtocol::Http => Utils::parse_http_url(&self.url),
            NetProtocol::Grpc => Utils::parse_http_url(&self.url),
            NetProtocol::WebSocket => Utils::parse_ws_url(&self.url),
            NetProtocol::Socket => Utils::parse_tcp_url(&self.url),
        }
    }
    pub fn to_config(&self) -> Result<NetConfig, NetResultStatus> {
        let addr: AddressInfo = self.to_protocol_address()?;
        Ok(NetConfig {
            addr,
            http: self.http.clone(),
            protocol: self.protocol,
            mode: self.mode,
            tls_mode: self.tls_mode,
            tor_config: self.tor_config.clone(),
            encoding: self.encoding,
        })
    }
    pub fn to_protocol_config(&self, protocol: NetProtocol) -> Result<NetConfig, NetResultStatus> {
        if self.protocol != protocol {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        self.to_config()
    }
}

/// C

#[repr(C)]
pub struct NetHttpHeaderC {
    pub key: *const c_char,
    pub value: *const c_char,
}
#[repr(C)]
pub struct NetHttpDigestAuthC {
    pub key: *const c_char,
    pub value: *const c_char,
}
#[repr(C)]
pub struct NetHttpConfigC {
    pub headers: *const NetHttpHeaderC,
    pub headers_len: u8,

    pub protocol: u8,
    pub global_client: bool, // nullable
}

#[repr(C)]
pub struct NetTorClientConfigC {
    pub cache_dir: *const c_char,
    pub state_dir: *const c_char,
}

#[repr(C)]
pub struct NetRequestConfigC {
    pub url: *const c_char,
    pub mode: u8,
    pub protocol: u8,
    pub http: *const NetHttpConfigC,
    pub tls_mode: u8,
    pub tor_config: *const NetTorClientConfigC,
    pub stream_encoding: u8,
}
impl TryFrom<&NetHttpHeaderC> for NetHttpHeader {
    type Error = NetResultStatus;
    fn try_from(c: &NetHttpHeaderC) -> Result<Self, NetResultStatus> {
        if c.key.is_null() || c.value.is_null() {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        Ok(Self {
            key: unsafe { Utils::cstr_to_string(c.key as *const u8) },
            value: unsafe { Utils::cstr_to_string(c.value as *const u8) },
        })
    }
}

impl TryFrom<&NetTorClientConfigC> for NetTorClientConfig {
    type Error = NetResultStatus;
    fn try_from(c: &NetTorClientConfigC) -> Result<Self, NetResultStatus> {
        if c.cache_dir.is_null() || c.state_dir.is_null() {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        Ok(Self {
            cache_dir: unsafe { Utils::cstr_to_string(c.cache_dir as *const u8) },
            state_dir: unsafe { Utils::cstr_to_string(c.state_dir as *const u8) },
        })
    }
}
// impl TryFrom<&NetHttpDigestAuthC> for NetHttpDigestAuth {
//     type Error = NetResultStatus;
//     fn try_from(c: &NetHttpDigestAuthC) -> Result<Self, NetResultStatus> {
//         if c.key.is_null() || c.value.is_null() {
//             return Err(NetResultStatus::InvalidConfigParameters);
//         }
//         Ok(Self {
//             key: unsafe { Utils::cstr_to_string(c.key as *const u8) },
//             value: unsafe { Utils::cstr_to_string(c.value as *const u8) },
//         })
//     }
// }
impl TryFrom<&NetHttpConfigC> for NetHttpConfig {
    type Error = NetResultStatus;
    fn try_from(c: &NetHttpConfigC) -> Result<Self, NetResultStatus> {
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
        Ok(Self {
            headers,
            protocol,
            global_client: c.global_client,
        })
    }
}
impl TryFrom<&NetRequestConfigC> for NetRequestConfig {
    type Error = NetResultStatus;
    fn try_from(c: &NetRequestConfigC) -> Result<Self, NetResultStatus> {
        let http = unsafe {
            c.http
                .as_ref()
                .map(NetHttpConfig::try_from)
                .transpose()?
                .ok_or(NetResultStatus::InvalidConfigParameters)?
        };
        let tor_config = unsafe {
            c.tor_config
                .as_ref()
                .map(NetTorClientConfig::try_from)
                .transpose()?
        };
        if c.url.is_null() {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        Ok(Self {
            url: unsafe { Utils::cstr_to_string(c.url as *const u8) },
            tor_config: tor_config,
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
                1 => TlsMode::Safe,
                2 => TlsMode::Dangerous,
                _ => return Err(NetResultStatus::InvalidConfigParameters),
            },
            encoding: match c.stream_encoding {
                1 => StreamEncoding::Json,
                2 => StreamEncoding::Raw,
                3 => StreamEncoding::CborJson,
                _ => return Err(NetResultStatus::InvalidConfigParameters),
            },
            http,
        })
    }
}
