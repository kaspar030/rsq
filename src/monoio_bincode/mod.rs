use bincode::{Decode, Encode};
use bytes::{BufMut, BytesMut};
use monoio_codec::{length_delimited::LengthDelimitedCodec, Decoded, Decoder, Encoder};
use std::{io, marker::PhantomData, sync::Arc};

pub struct BincodeCodec<T> {
    inner: LengthDelimitedCodec,
    _phantom: PhantomData<T>,
}

impl<T> BincodeCodec<T> {
    pub fn new() -> Self {
        let inner = LengthDelimitedCodec::default();
        Self {
            inner,
            _phantom: PhantomData,
        }
    }
}

impl<T: Decode<()>> Decoder for BincodeCodec<T> {
    type Item = Arc<T>;

    type Error = io::Error;

    fn decode(
        &mut self,
        src: &mut bytes::BytesMut,
    ) -> Result<monoio_codec::Decoded<Self::Item>, Self::Error> {
        match self.inner.decode(src) {
            Ok(Decoded::Some(bytes)) => {
                let (data, _size) =
                    bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
                Ok(Decoded::Some(data))
            }
            Ok(Decoded::Insufficient) => Ok(Decoded::Insufficient),
            Ok(Decoded::InsufficientAtLeast(n)) => Ok(Decoded::InsufficientAtLeast(n)),
            Err(e) => Err(e),
        }
    }
}

struct ByteMutBincodeWriter<'a> {
    bytes: &'a mut BytesMut,
}

impl bincode::enc::write::Writer for ByteMutBincodeWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}

struct BincodeCountingWriter {
    written: usize,
}
impl bincode::enc::write::Writer for BincodeCountingWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        self.written += bytes.len();
        Ok(())
    }
}

impl<T: Encode> Encoder<Arc<T>> for BincodeCodec<T> {
    type Error = io::Error;

    fn encode(&mut self, data: Arc<T>, dst: &mut BytesMut) -> Result<(), io::Error> {
        let mut counter = BincodeCountingWriter { written: 0 };
        bincode::encode_into_writer(&data, &mut counter, bincode::config::standard())
            .expect("encoding went well");

        dst.reserve(counter.written + 4);
        dst.put_u32(counter.written as u32);

        let writer = ByteMutBincodeWriter { bytes: dst };
        bincode::encode_into_writer(&data, writer, bincode::config::standard())
            .expect("encoding went well");

        Ok(())
    }
}
