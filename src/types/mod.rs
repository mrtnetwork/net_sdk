use std::sync::Arc;

use crate::types::response::NetResponseKind;

pub mod config;
pub mod error;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(target_arch = "wasm32")]
pub mod request;
pub mod response;

#[derive(Debug, Clone)]
pub struct AddressInfo {
    pub host: String,
    pub url: String,
    pub port: u16,
    pub is_tls: bool,
}

#[cfg(target_arch = "wasm32")]
pub type DartCallback = Arc<dyn Fn(NetResponseKind) + 'static>; // no Send/Sync

#[cfg(not(target_arch = "wasm32"))]
pub type DartCallback = Arc<dyn Fn(NetResponseKind) + Send + Sync + 'static>;
