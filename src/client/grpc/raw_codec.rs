use bytes::{Buf, BufMut, Bytes};
use tonic::{
    Status,
    codec::{BufferSettings, Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
};
#[derive(Default, Clone)]
pub struct BufferCodec;
impl Codec for BufferCodec {
    type Decode = Vec<u8>;
    type Encode = Vec<u8>;

    type Encoder = BufferRawBytesEncoder;
    type Decoder = BufferRawBytesDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        BufferRawBytesEncoder
    }

    fn decoder(&mut self) -> Self::Decoder {
        BufferRawBytesDecoder
    }
}
#[derive(Default, Clone)]
pub struct BufferRawBytesEncoder;

impl Encoder for BufferRawBytesEncoder {
    type Item = Vec<u8>;
    type Error = Status;

    fn encode(&mut self, item: Vec<u8>, dst: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
        dst.put(Bytes::from(item));
        Ok(())
    }

    fn buffer_settings(&self) -> BufferSettings {
        BufferSettings::default()
    }
}
#[derive(Default, Clone)]
pub struct BufferRawBytesDecoder;

impl Decoder for BufferRawBytesDecoder {
    type Item = Vec<u8>;
    type Error = Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        Ok(Some(src.copy_to_bytes(src.remaining()).to_vec()))
    }

    fn buffer_settings(&self) -> BufferSettings {
        BufferSettings::default()
    }
}
