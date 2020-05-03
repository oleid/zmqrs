use nom::{
    number::complete::{be_u64, be_u8},
    IResult,
};

use crate::{command, message, Command, Message};

#[derive(Debug, Default)]
pub struct FrameFlags {
    /// A value of 1 indicates that the frame is a command frame.
    /// A value of 0 indicates that the frame is a message frame.
    pub is_command: bool,

    /// value of 0 indicates that the frame size is encoded as a single octet.
    /// A value of 1 indicates that the frame size is encoded as a 64-bit unsigned integer in
    /// network byte order.
    pub is_long: bool,

    /// A value of 0 indicates that there are no more frames to follow.
    /// A value of 1 indicates that more frames will follow. This bit SHALL be zero on command frames.
    pub more_frames_to_follow: bool,
}

impl FrameFlags {
    pub fn from_byte(head_byte: u8) -> FrameFlags {
        let more_frames_to_follow = head_byte & 1u8 << 0 != 0;
        let is_long = head_byte & 1u8 << 1 != 0;
        let is_command = head_byte & 1u8 << 2 != 0;

        FrameFlags {
            more_frames_to_follow,
            is_long,
            is_command,
        }
    }
}
#[derive(Debug, Default)]
pub struct FrameHeader {
    pub flags: FrameFlags,
    pub frame_length: usize,
}

#[derive(Debug, Clone)]
pub enum Frame<S, T> {
    Command(Command<S, T>),
    Message(Message<T>),
}

impl From<(&bytes::Bytes, Frame<&str, &[u8]>)> for Frame<bytes::Bytes, bytes::Bytes> {
    fn from(input: (&bytes::Bytes, Frame<&str, &[u8]>)) -> Self {
        let (buffer, frame) = input;

        match frame {
            Frame::Command(c) => Frame::Command(((buffer, c)).into()),
            Frame::Message(m) => Frame::Message(((buffer, m)).into()),
        }
    }
}

pub fn frame_header<'a>(
    input: &'a [u8],
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], FrameHeader> {
    let (input, head_byte) = be_u8(input)?;
    let flags = FrameFlags::from_byte(head_byte);

    let (input, frame_length) = if flags.is_long {
        be_u64(input).map(|(input, v)| (input, v as usize))?
    } else {
        be_u8(input).map(|(input, v)| (input, v as usize))?
    };

    trace!(logger, "frame_header:";
        o!("more_frames_to_follow" => flags.more_frames_to_follow),
        o!("is_long" => flags.is_long),
        o!("is_command" => flags.is_command),
        o!("frame_length" => frame_length));

    Ok((
        input,
        FrameHeader {
            flags,
            frame_length,
        },
    ))
}

pub fn frame_body<'a>(
    input: &'a [u8],
    hdr: &FrameHeader,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Frame<&'a str, &'a [u8]>> {
    if hdr.flags.is_command {
        let (input, cmd) = command(input, &hdr, logger)?;
        Ok((input, Frame::Command(cmd)))
    } else {
        let (input, msg) = message(input, &hdr, logger)?;
        Ok((input, Frame::Message(msg)))
    }
}

pub fn frame<'a>(
    input: &'a [u8],
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Frame<&'a str, &'a [u8]>> {
    let (input, hdr) = frame_header(input, logger)?;

    frame_body(input, &hdr, logger)
}
