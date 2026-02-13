use rustls::pki_types::ServerName;
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

    pub fn get_server_name(host: &str) -> Result<ServerName<'static>, NetResultStatus> {
        ServerName::try_from(host.to_owned()).map_err(|_| NetResultStatus::InvalidUrl)
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

    pub unsafe fn string_to_c_ptr(s: &str) -> *mut u8 {
        let len = s.len();

        let buf = unsafe { libc::malloc(len + 1) } as *mut u8;
        if buf.is_null() {
            return std::ptr::null_mut();
        }

        unsafe { std::ptr::copy_nonoverlapping(s.as_ptr(), buf, len) };
        unsafe { *buf.add(len) = 0 };

        buf
    }

    pub unsafe fn free_c_string(ptr: *mut u8) {
        if !ptr.is_null() {
            unsafe { libc::free(ptr as *mut libc::c_void) };
        }
    }
}
