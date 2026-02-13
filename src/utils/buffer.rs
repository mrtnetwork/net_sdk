use serde_cbor;
use serde_json::Value;
use std::str;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum StreamEncoding {
    Json = 1,
    Raw = 2,
    CborJson = 3,
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
        for byte in b {
            self.buffer.push(byte);

            if let Ok(s) = str::from_utf8(&self.buffer) {
                if serde_json::from_str::<Value>(s).is_ok() {
                    let result = self.buffer.clone();
                    self.buffer.clear();
                    return Some(result);
                }
            }
        }
        None
    }

    /// Add bytes according to encoding
    pub fn add(&mut self, buf: Vec<u8>) -> Option<Vec<u8>> {
        match self.encoding {
            StreamEncoding::Raw => Some(buf),

            StreamEncoding::Json => self.is_json(buf),

            StreamEncoding::CborJson => {
                if let Some(json_bytes) = self.is_json(buf) {
                    // convert JSON string to CBOR bytes
                    let s = std::str::from_utf8(&json_bytes).ok()?;
                    let v: Value = serde_json::from_str(s).ok()?;
                    let cbor_bytes = serde_cbor::to_vec(&v).ok()?;
                    Some(cbor_bytes)
                } else {
                    None
                }
            }
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

            StreamEncoding::CborJson => {
                if let Ok(s) = std::str::from_utf8(&buf) {
                    if let Ok(v) = serde_json::from_str::<Value>(s) {
                        if let Ok(cbor_bytes) = serde_cbor::to_vec(&v) {
                            return (cbor_bytes, encoding);
                        }
                    }
                }
                // fallback: raw bytes
                (buf, StreamEncoding::Raw)
            }
        }
    }
}
