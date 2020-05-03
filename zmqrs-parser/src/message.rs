use nom::{bytes::complete::take, IResult};

use crate::{ByteSlice, FrameHeader};

#[derive(Debug, Clone)]
pub struct Message<T>(
    // TODO: think about what to do here. Since we can have multi-part messages,
    // maybe some kind of Vec<&[u8]> makes sense to copy only little data
    ByteSlice<T>,
);

impl From<(&bytes::Bytes, Message<&[u8]>)> for Message<bytes::Bytes> {
    fn from(input: (&bytes::Bytes, Message<&[u8]>)) -> Self {
        let (buffer, subset) = input;
        Message((buffer, subset.0).into())
    }
}

/// Parse a single message
///
/// Messages carry application data and are not generally created, modified, or filtered by the ZMTP
/// implementation except in some cases. Messages consist of one or more frames and an
/// implementation SHALL always send and deliver messages atomically, that is, all the frames of a
/// message, or none of them.
pub fn message<'a>(
    input: &'a [u8],
    hdr: &FrameHeader,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Message<&'a [u8]>> {
    // TODO: Think about multi-frame messages
    let (input, msg) = take(hdr.frame_length)(input)?;
    trace!(logger, "message:";
        o!("length" => msg.len()),
        o!("content" => ByteSlice(msg)));
    Ok((input, Message(ByteSlice(msg))))
}
