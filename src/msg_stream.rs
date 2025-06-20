use std::{convert::TryInto, io::ErrorKind};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use monoio::{
    buf::{IoBufMut, SliceMut},
    io::{AsyncBufRead, AsyncReadRent, AsyncReadRentExt, BufReader},
    BufResult,
};

use crate::messaging::msg::{ChannelMsgHdr, Msg};

pub struct FrameDecoder<IO> {
    io: BufReader<IO>,
}

// trait PeekBufReader {
//     async fn peek(&mut self, &mut [u8]) -> std::io::Result<()>;
// }

// impl<R: AsyncReadRent> PeekBufReader for BufReader<R> {
//     async fn peek(&mut self, target: &mut [u8]) -> std::io::Result<()> {
//         let mut nread = 0;
//         while nread < target.len() {
//             let buffered = self.fill_buf().await?;
//             let available = buffered.len();
//             let remaining = target.len() - nread;
//             let to_copy = available.min(remaining);
//             target[nread..].copy_from_slice(&buffered[..to_copy]);
//             nread += available;
//         }
//         Ok(())
//     }
// }

impl<IO> FrameDecoder<IO> {
    pub fn new(io: IO) -> Self {
        Self {
            io: BufReader::new(io),
        }
    }

    pub async fn next(&mut self) -> Option<std::io::Result<BytesMut>>
    where
        IO: AsyncReadRent,
    {
        let mut buf = BytesMut::with_capacity(128);
        let (size, nread) = {
            let size: u32;
            let nread: usize;
            let buf_content = self.io.fill_buf().await.unwrap();
            if buf_content.len() >= size_of::<u32>() {
                size = u32::from_be_bytes(buf_content[0..4].try_into().unwrap());
                nread = 0;
            } else {
                let (res, buf_back) = self.read_exact_n(buf, 4).await;
                buf = buf_back;
                nread = match res {
                    Ok(0) => return None,
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return None,
                    Err(e) => return Some(Err(e)),
                    Ok(n) => n,
                };

                size = u32::from_be_bytes(buf[0..4].try_into().unwrap());
            }
            (size, nread)
        };

        //tracing::info!("2. got size={size}");
        let whole_frame_size = size + 4;
        let to_read = whole_frame_size as usize - nread;

        buf.reserve(to_read);

        // tracing::info!(
        //     "buf len:{} whole_frame_size={whole_frame_size} to_read={to_read}",
        //     buf.len()
        // );
        let res = {
            let rest = buf.split_off(nread);
            let (res, rest) = self.read_exact_n(rest, to_read).await;
            buf.unsplit(rest);
            res
        };

        let nread = match res {
            Ok(0) => return None,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return None,
            Err(e) => return Some(Err(e)),
            Ok(n) => n,
        } + nread;

        Some(Ok(buf))
    }

    async fn read_exact_n<T: IoBufMut + 'static>(
        &mut self,
        mut buf: T,
        len: usize,
    ) -> BufResult<usize, T>
    where
        IO: AsyncReadRent,
    {
        let mut read = 0;

        while read < len {
            let buf_slice = unsafe { SliceMut::new_unchecked(buf, read, len) };

            let (result, buf_slice) = self.io.read(buf_slice).await;

            buf = buf_slice.into_inner();

            match result {
                Ok(0) => {
                    return (
                        Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "failed to fill whole buffer",
                        )),
                        buf,
                    )
                }

                Ok(n) => {
                    read += n;

                    unsafe { buf.set_init(read) };
                }

                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}

                Err(e) => return (Err(e), buf),
            }
        }

        (Ok(read), buf)
    }
}
