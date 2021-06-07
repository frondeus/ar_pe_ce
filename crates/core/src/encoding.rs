use std::marker::PhantomData;

use anyhow::Context;
use bytes::{Buf, BufMut, BytesMut};
use serde::{de::DeserializeOwned, Serialize};
use tokio_util::codec::{Decoder, Encoder};

use anyhow::{Error, Result};

pub type DefaultCodec<T> = Json<T>;

// pub type DefaultCodec<T> = MsgPack<T>;

pub struct Json<T> {
    _phantom: PhantomData<T>,
}

impl<T> Default for Json<T> {
    fn default() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

impl<T> Decoder for Json<T>
where
    T: DeserializeOwned,
{
    type Item = T;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        if src.len() < 4 {
            return Ok(None);
        }

        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&src[..4]);
        let length = u32::from_le_bytes(length_bytes) as usize;

        if length > u32::MAX as usize {
            anyhow::bail!("Frame length {} is too large", length);
        }

        if src.len() < 4 + length {
            src.reserve(4 + length - src.len());

            return Ok(None);
        }

        let data = src[4..4 + length].to_vec();
        src.advance(4 + length);
        tracing::warn!("Decoding: {}", std::str::from_utf8(&data).unwrap());
        let msg = serde_json::from_slice(&data).context("Could not deserialize message")?;
        Ok(Some(msg))
    }
}

use std::fmt::Debug;

impl<T> Encoder<T> for Json<T>
where
    T: Serialize + Debug,
{
    type Error = Error;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<()> {
        let mut buf = BytesMut::with_capacity(8 * 1024).writer();
        tracing::warn!(?item, "Encoding");
        if let Err(e) =
            serde_json::to_writer(&mut buf, &item).context("Could not serialize message")
        {
            anyhow::bail!("Could not encode: {}", e);
        }

        let buf_len = buf.get_ref().len();

        if buf_len > u32::MAX as usize {
            anyhow::bail!("Frame of length {} is too large", buf_len);
        }

        let len_slice = u32::to_le_bytes(buf_len as u32);

        dst.reserve(4 + buf_len);

        dst.extend_from_slice(&len_slice);
        dst.extend_from_slice(buf.get_ref());
        Ok(())
    }
}

// struct MsgPack<T> {
//     _phantom: PhantomData<T>,
// }

// impl<T> MsgPack<T> {
//     pub fn new() -> Self {
//         Self {
//             _phantom: Default::default(),
//         }
//     }
// }

// impl<T: DeserializeOwned> Decoder for MsgPack<T> {
//     type Item = T;
//     type Error = Error;

//     fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
//         if src.len() < 4 {
//             return Ok(None);
//         }

//         let mut length_bytes = [0u8; 4];
//         length_bytes.copy_from_slice(&src[..4]);
//         let length = u32::from_le_bytes(length_bytes) as usize;

//         if length > u32::MAX as usize {
//             anyhow::bail!("Frame length {} is too large", length);
//         }

//         if src.len() < 4 + length {
//             src.reserve(4 + length - src.len());

//             return Ok(None);
//         }

//         let data = src[4..4 + length].to_vec();
//         src.advance(4 + length);

//         let msg = rmp_serde::from_read_ref(&data).context("Could not deserialize message")?;
//         Ok(Some(msg))
//     }
// }

// impl<T: Serialize> Encoder<T> for MsgPack<T> {
//     type Error = Error;

//     fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<()> {
//         let mut buf = BytesMut::with_capacity(8 * 1024).writer();
//         if let Err(e) =
//             rmp_serde::encode::write(&mut buf, &item).context("Could not serialize message")
//         {
//             anyhow::bail!("Could not encode: {}", e)
//         }

//         let buf_len = buf.get_ref().len();

//         if buf_len > u32::MAX as usize {
//             anyhow::bail!("Frame of length {} is too large", buf_len);
//         }

//         let len_slice = u32::to_le_bytes(buf_len as u32);

//         dst.reserve(4 + buf_len);

//         dst.extend_from_slice(&len_slice);
//         dst.extend_from_slice(buf.get_ref());
//         Ok(())
//     }
// }
