#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetHttpProtocol {
    Http1 = 1,
    Http2 = 2,
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
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
pub enum NetTlsMode {
    Safe = 1,
    Dangerous = 2,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug)]
pub struct NetHttpHeader {
    key: String,
    value: String,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetHttpHeader {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn create(key: String, value: String) -> Self {
        Self { key, value }
    }
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn key(&self) -> String {
        self.key.clone()
    }
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn value(&self) -> String {
        self.value.clone()
    }
}

impl NetHttpHeader {
    pub fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
    pub fn key_ref(&self) -> &str {
        &self.key
    }

    pub fn value_ref(&self) -> &str {
        &self.value
    }
}
#[derive(Clone, Debug)]
pub struct NetConfigHttp {
    pub headers: Vec<NetHttpHeader>,
    pub protocol: Option<NetHttpProtocol>,
}
#[derive(Clone, Debug)]
pub struct NetConfigTor {
    pub cache_dir: String,
    pub state_dir: String,
}
#[derive()]
pub struct NetConfigRequest {
    pub url: String,
    pub mode: NetMode,
    pub protocol: NetProtocol,
    pub http: NetConfigHttp,
    pub tls_mode: NetTlsMode,
    // pub tor_config: Option<NetConfigTor>,
    pub encoding: StreamEncoding,
}

#[derive(Clone, Debug)]
pub struct NetConfig {
    pub addr: AddressInfo,
    pub mode: NetMode,
    pub http: NetConfigHttp,
    pub protocol: NetProtocol,
    pub tls_mode: NetTlsMode,
    // pub tor_config: Option<NetConfigTor>,
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
            encoding: self.encoding.clone(),
        }
    }
}

impl Default for NetConfigHttp {
    fn default() -> NetConfigHttp {
        Self {
            headers: Vec::new(),
            protocol: None,
        }
    }
}

impl NetConfigRequest {
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug)]
pub struct NetConfigHttpWasm {
    headers: Vec<NetHttpHeader>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetConfigHttpWasm {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn create(headers: Vec<NetHttpHeader>) -> Self {
        Self { headers }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug)]
pub struct NetConfigRequestWasm {
    url: String,
    protocol: NetProtocol,
    http: NetConfigHttpWasm,
    encoding: StreamEncoding,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetConfigRequestWasm {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn create(
        url: String,
        protocol: NetProtocol,
        http: NetConfigHttpWasm,
        encoding: StreamEncoding,
    ) -> Self {
        Self {
            url,
            protocol,
            http,
            encoding,
        }
    }
}
// Rust-side method to convert to NetConfig
impl NetConfigRequestWasm {
    pub fn to_config(&self) -> Result<NetConfigRequest, NetResultStatus> {
        // convert NetConfigHttpWasm → NetConfigHttp
        let http = NetConfigHttp {
            headers: self.http.headers.clone(),
            protocol: None, // map if needed
        };

        Ok(NetConfigRequest {
            url: self.url.clone(),
            mode: NetMode::Clearnet,
            protocol: self.protocol,
            tls_mode: NetTlsMode::Safe,
            http,
            encoding: self.encoding,
        })
    }
}
