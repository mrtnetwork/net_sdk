#[cfg(not(target_arch = "wasm32"))]
pub mod grpc;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
mod tls;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
