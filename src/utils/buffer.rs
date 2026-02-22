use serde_json::Value;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum StreamEncoding {
    Json = 1,
    Raw = 2,
}

pub struct StreamBuffer {
    encoding: StreamEncoding,
    buffer: Vec<u8>,
}

impl StreamBuffer {
    pub fn new(encoding: StreamEncoding) -> Self {
        Self {
            encoding,
            buffer: Vec::new(),
        }
    }

    /// Try to parse JSON incrementally
    fn is_json(&mut self, b: Vec<u8>) -> Option<Vec<u8>> {
        if self.buffer.is_empty() {
            if let Ok(s) = std::str::from_utf8(&b) {
                if serde_json::from_str::<serde_json::Value>(s).is_ok() {
                    return Some(b);
                }
            }
            self.buffer.extend_from_slice(&b);
            return None;
        }
        self.buffer.extend_from_slice(&b);
        if let Ok(s) = std::str::from_utf8(&self.buffer) {
            if serde_json::from_str::<serde_json::Value>(s).is_ok() {
                let result = self.buffer.clone();
                self.buffer.clear();
                return Some(result);
            }
        }

        None
    }

    /// Add bytes according to encoding
    pub fn add(&mut self, buf: Vec<u8>) -> Option<Vec<u8>> {
        match self.encoding {
            StreamEncoding::Raw => Some(buf),

            StreamEncoding::Json => self.is_json(buf),
        }
    }

    /// Try to interpret current buffer according to encoding.
    /// Returns (bytes, encoding actually detected)
    pub fn try_current_buffer(buf: Vec<u8>, encoding: StreamEncoding) -> (Vec<u8>, StreamEncoding) {
        match encoding {
            StreamEncoding::Raw => (buf, StreamEncoding::Raw),

            StreamEncoding::Json => {
                if let Ok(s) = std::str::from_utf8(&buf) {
                    if serde_json::from_str::<Value>(s).is_ok() {
                        return (buf, encoding);
                    }
                }
                // fallback: raw bytes
                (buf, StreamEncoding::Raw)
            }
        }
    }
}
