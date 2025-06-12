use bytes::{BufMut, BytesMut};
use monoio_codec::{length_delimited::LengthDelimitedCodec, Decoded, Decoder, Encoder};
use serde::{Deserialize, Serialize};
use std::{io, marker::PhantomData};

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

impl<T: for<'a> Deserialize<'a>> Decoder for BincodeCodec<T> {
    type Item = T;

    type Error = io::Error;

    fn decode(
        &mut self,
        src: &mut bytes::BytesMut,
    ) -> Result<monoio_codec::Decoded<Self::Item>, Self::Error> {
        match self.inner.decode(src) {
            Ok(Decoded::Some(bytes)) => {
                let (data, _size) =
                    bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
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

impl<T: Serialize> Encoder<T> for BincodeCodec<T> {
    type Error = io::Error;

    fn encode(&mut self, data: T, dst: &mut BytesMut) -> Result<(), io::Error> {
        let mut payload = dst.split_off(dst.len() + 4);

        let writer = ByteMutBincodeWriter {
            bytes: &mut payload,
        };

        bincode::serde::encode_into_writer(data, writer, bincode::config::standard())
            .expect("encoding went well");

        dst.put_u32(payload.len() as u32);
        dst.unsplit(payload);

        Ok(())
    }
}
