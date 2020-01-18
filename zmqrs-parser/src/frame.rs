use nom::{
    number::complete::{be_u64, be_u8},
    IResult,
};

use crate::{command, message, Command, Message};

#[derive(Debug, Default)]
pub struct FrameHeader {
    /// A value of 1 indicates that the frame is a command frame.
    /// A value of 0 indicates that the frame is a message frame.
    is_command: bool,

    /// value of 0 indicates that the frame size is encoded as a single octet.
    /// A value of 1 indicates that the frame size is encoded as a 64-bit unsigned integer in
    /// network byte order.
    is_long: bool,
    /// A value of 0 indicates that there are no more frames to follow.
    /// A value of 1 indicates that more frames will follow. This bit SHALL be zero on command frames.
    more_frames_to_follow: bool,

    pub frame_length: usize,
}

#[derive(Debug)]
pub enum Frame<'a> {
    Command(Command<'a>),
    Message(Message<'a>),
}

fn frame_header<'a>(input: &'a [u8], logger: &mut slog::Logger) -> IResult<&'a [u8], FrameHeader> {
    let (input, head_byte) = be_u8(input)?;
    let more_frames_to_follow = head_byte & 1u8 << 0 != 0;
    let is_long = head_byte & 1u8 << 1 != 0;
    let is_command = head_byte & 1u8 << 2 != 0;

    let (input, frame_length) = if is_long {
        be_u64(input).map(|(input, v)| (input, v as usize))?
    } else {
        be_u8(input).map(|(input, v)| (input, v as usize))?
    };

    trace!(logger, "frame_header:";
        o!("more_frames_to_follow" => more_frames_to_follow),
        o!("is_long" => is_long),
        o!("is_command" => is_command),
        o!("frame_length" => frame_length));

    Ok((
        input,
        FrameHeader {
            more_frames_to_follow,
            is_long,
            is_command,
            frame_length,
        },
    ))
}

pub fn frame<'a>(input: &'a [u8], logger: &mut slog::Logger) -> IResult<&'a [u8], Frame<'a>> {
    let (input, hdr) = frame_header(input, logger)?;

    if hdr.is_command {
        let (input, cmd) = command(input, &hdr, logger)?;
        Ok((input, Frame::Command(cmd)))
    } else {
        let (input, msg) = message(input, &hdr, logger)?;
        Ok((input, Frame::Message(msg)))
    }
}
