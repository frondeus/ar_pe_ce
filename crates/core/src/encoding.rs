use anyhow::Context;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::stream::{StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
enum DecodeState {
    ReadHeader,
    ReadBody { len: usize },
}

fn decode_chunk<T>(buf: &mut BytesMut, state: &mut DecodeState) -> Result<Option<T>>
where
    T: DeserializeOwned + Send,
{
    if *state == DecodeState::ReadHeader {
        if buf.remaining() < 4 {
            return Ok(None);
        }

        let len = buf.get_u32() as usize;
        buf.reserve(len);
        *state = DecodeState::ReadBody { len };
    }

    if let DecodeState::ReadBody { len } = *state {
        if buf.remaining() < len || buf.len() < len {
            return Ok(None);
        }

        let to_decode = buf.split_to(len).freeze();

        let msg = rmp_serde::from_read_ref(&to_decode).context("Could not deserialize message")?;

        *state = DecodeState::ReadHeader;
        return Ok(Some(msg));
    }

    Ok(None)
}

pub fn decode_stream<T>(
    stream: impl futures::Stream<Item = Result<Bytes, hyper::Error>> + Send + 'static,
) -> impl futures::Stream<Item = Result<T>> + Send
where
    T: DeserializeOwned + Send,
{
    let mut buf = BytesMut::with_capacity(8 * 1024);
    let mut state = DecodeState::ReadHeader;

    async_stream::stream! {
        futures::pin_mut!(stream);

        loop {
            match decode_chunk(&mut buf, &mut state) {
                Err(e) => {
                    yield Err(e);
                    continue;
                },
                Ok(Some(t)) => {
                    yield Ok(t);
                    continue;
                }
                Ok(None) => ()
            }

            match stream.next().await {
                Some(Ok(bytes)) => {

                    buf.put(bytes);



                },
                Some(Err(e)) => yield Err(Error::from(e)),
                None => break,
            }
        }
    }
}

pub fn encode_stream<T>(
    stream: impl futures::Stream<Item = Result<T>> + Send + 'static,
) -> impl futures::Stream<Item = Result<Bytes>> + Send + 'static
where
    T: Serialize,
{
    let mut buf = BytesMut::with_capacity(8 * 1024).writer();
    stream.and_then(move |msg| {
        futures::future::ready({
            buf.get_mut().reserve(4);
            unsafe {
                buf.get_mut().advance_mut(4);
            }

            if let Err(e) =
                rmp_serde::encode::write(&mut buf, &msg).context("Could not serialize message")
            {
                return futures::future::ready(Result::Err(e));
            }

            let len = buf.get_ref().len() - 4;
            assert!(len <= std::u32::MAX as usize);
            {
                let buf = buf.get_mut();
                let mut buf = &mut buf[..4];
                buf.put_u32(len as u32);
            }

            let encoded = buf.get_mut().split_to(len + 4).freeze();
            Ok(encoded)
        })
    })
}
