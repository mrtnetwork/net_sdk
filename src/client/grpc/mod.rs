#[cfg(not(target_arch = "wasm32"))]
pub mod native;

pub mod raw_codec;
#[cfg(target_arch = "wasm32")]
pub mod wasm;
