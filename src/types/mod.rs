use std::sync::Arc;

use crate::types::response::NetResponseKind;

pub mod config;
pub mod error;
pub mod request;
pub mod response;
#[derive(Debug, Clone)]
pub struct AddressInfo {
    pub host: String,
    pub url: String,
    pub port: u16,
    pub is_tls: bool,
}

pub type DestroyFn = Arc<dyn Fn() + Send + Sync>;
pub type DartCallback = Arc<dyn Fn(NetResponseKind) + Send + Sync + 'static>;
