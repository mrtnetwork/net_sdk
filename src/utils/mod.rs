use url::Url;

use crate::types::{AddressInfo, error::NetResultStatus};

pub struct Utils;
pub mod buffer;

impl Utils {
    // This is now a "static" method on Utils
    pub fn parse_ws_url(url_str: &str) -> Result<AddressInfo, NetResultStatus> {
        let url = Url::parse(url_str).map_err(|_| NetResultStatus::InvalidUrl)?;
        let is_tls = match url.scheme() {
            "ws" => false,
            "wss" => true,
            _ => return Err(NetResultStatus::InvalidUrl),
        };
        let host = url
            .host_str()
            .ok_or_else(|| NetResultStatus::InvalidUrl)?
            .to_string();
        let port = url.port().unwrap_or_else(|| if is_tls { 443 } else { 80 });
        Ok(AddressInfo {
            host,
            port,
            is_tls,
            url: url_str.to_string(),
        })
    }

    pub fn parse_tcp_url(url_str: &str) -> Result<AddressInfo, NetResultStatus> {
        let url = Url::parse(url_str).map_err(|_| NetResultStatus::InvalidUrl)?;
        let is_tls = match url.scheme() {
            "tcp" => false,
            "tls" | "tcp+tls" => true,
            _ => return Err(NetResultStatus::InvalidUrl),
        };
        let port = url.port().unwrap_or_else(|| if is_tls { 443 } else { 80 });
        let host = url
            .host_str()
            .ok_or_else(|| NetResultStatus::InvalidUrl)?
            .to_string();
        Ok(AddressInfo {
            host,
            port,
            is_tls,
            url: url_str.to_string(),
        })
    }

    pub fn parse_http_url(url_str: &str) -> Result<AddressInfo, NetResultStatus> {
        let url = Url::parse(url_str).map_err(|_| NetResultStatus::InvalidUrl)?;
        let is_tls = match url.scheme() {
            "http" => false,
            "https" => true,
            _ => return Err(NetResultStatus::InvalidUrl),
        };
        let port = url.port().unwrap_or_else(|| if is_tls { 443 } else { 80 });
        let host = url
            .host_str()
            .ok_or_else(|| NetResultStatus::InvalidUrl)?
            .to_string();
        Ok(AddressInfo {
            host,
            port,
            is_tls,
            url: url_str.to_string(),
        })
    }

    pub unsafe fn cstr_to_string(ptr: *const u8) -> String {
        if ptr.is_null() {
            return String::new();
        }
        let mut len = 0;
        while unsafe { *ptr.add(len) } != 0 {
            len += 1;
        }
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        String::from_utf8_lossy(slice).into_owned()
    }
    /// Convert a null-terminated C string to a byte slice (`&[u8]`) without copying
    pub unsafe fn cstr_to_slice<'a>(ptr: *const u8) -> &'a [u8] {
        if ptr.is_null() {
            return &[];
        }
        let mut len = 0;
        while unsafe { *ptr.add(len) } != 0 {
            len += 1;
        }
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    /// Optionally convert to str without copying (lifetime tied to Dart memory)
    pub unsafe fn cstr_to_str<'a>(ptr: *const u8) -> &'a str {
        std::str::from_utf8(unsafe { Utils::cstr_to_slice(ptr) }).unwrap_or("")
    }
}
