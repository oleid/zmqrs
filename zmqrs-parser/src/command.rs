use nom::{
    bytes::complete::{take, take_while_m_n},
    number::complete::{be_u16, be_u32, be_u8},
    IResult,
};

use alloc::collections::BTreeMap as Map;
use slog::{Error, Record, Serializer};

use crate::{ByteSlice, FrameHeader};

impl<'a> slog::Value for ByteSlice<'a> {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> Result<(), Error> {
        let to_printable_ascii = |v: u8| if v >= 32 && v < 127 { v } else { b'.' };

        let mut buf = [0u8; 80]; // small internal buffer to sanitize bytes to something printable
        let mut cnt = 0;
        for (v, k) in self.0.iter().zip(buf.iter_mut()) {
            *k = to_printable_ascii(*v);
            cnt += 1;
        }
        serializer.emit_str(
            key,
            core::str::from_utf8(&buf[..cnt]).unwrap_or("<cannot display>"),
        )
    }
}

// http://zmtp.org/page:read-the-docs#toc12
#[derive(Debug)]
pub enum Command<'a> {
    // for null-security
    READY(MetaData<'a>),
    ERROR(ByteSlice<'a>),
    SUBSCRIBE(ByteSlice<'a>),
    CANCEL(ByteSlice<'a>),
    PING(Ping<'a>),
    PONG(Pong<'a>),
}

#[derive(Debug)]
pub struct Ping<'a> {
    pub ttl: u16,
    pub context: &'a [u8],
}

#[derive(Debug)]
pub struct Pong<'a> {
    pub context: &'a [u8],
}

#[derive(Debug)]
pub struct MetaData<'a> {
    /// Metadata names SHALL be case-insensitive.
    /// These metadata properties are defined:
    ///
    /// * "Socket-Type", which specifies the sender's socket type. See the section "The Socket Type Property" below. The sender SHOULD specify the Socket-Type.
    ///
    /// * "Identity", which specifies the sender's socket identity. See the section "The Identity Property" below. The sender MAY specify an Identity.
    ///
    /// * "Resource", which specifies the a resource to connect to. See the section "The Resource Property" below. The sender MAY specify a Resource.
    properties: Map<&'a str, ByteSlice<'a>>, // TODO: das passt für plain, aber auch für andere?
}

fn meta_data<'a>(
    input: &'a [u8],
    data_len: usize,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], MetaData<'a>> {
    let mut properties = Map::new();

    let mut current_pos = input;

    while input.len() - current_pos.len() < data_len {
        let (new_pos, (name, value)) = property(current_pos, logger)?;

        trace!(logger, "property";
                "consumed" => input.len() - new_pos.len(),
                "remaining" => data_len - (input.len() - new_pos.len()),
                "data_len" => data_len );

        current_pos = new_pos;
        properties.insert(name, value);
    }
    Ok((input, MetaData { properties }))
}

/// Parse a single property
///
/// property = name value
/// name = short-size 1*255name-char
/// name-char = ALPHA | DIGIT | "-" | "_" | "." | "+"
/// value = 4OCTET *OCTET       ; Size in network byte order
fn property<'a>(
    input: &'a [u8],
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], (&'a str, ByteSlice<'a>)> {
    let is_name_char = |v: u8| match v {
        b'-' => true,
        b'_' => true,
        b'.' => true,
        b'+' => true,
        _ => nom::character::is_alphanumeric(v),
    };
    let (input, name_len) = be_u8(input)?;
    let (input, name_raw) = take_while_m_n(1, name_len as usize, is_name_char)(input)?;
    let (input, value_len) = be_u32(input)?;
    let (input, value) = take(value_len as usize)(input)?;

    // If this conversion ever causes performance problems, it could be replaced with an unsafe
    // variant. The constraint "is_name_char" is stronger than utf8 validity.
    let name = core::str::from_utf8(name_raw).unwrap_or("<this cannot happen>");

    trace!(logger, "property"; "name" => name, "value" => ByteSlice(value) );

    Ok((input, (name, ByteSlice(value))))
}

pub fn command<'a>(
    input: &'a [u8],
    hdr: &FrameHeader,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Command<'a>> {
    let (input, cmd_name_len) = be_u8(input)?;

    // sanity check: longest command name is SUBSCRIBE
    if cmd_name_len > 10 {
        Err(nom::Err::Error(nom::error::make_error(
            input,
            nom::error::ErrorKind::LengthValue,
        )))
    } else {
        let (cmd_name, remaining) = input.split_at(cmd_name_len as usize);

        assert!(hdr.frame_length >= cmd_name.len() + 1);
        let data_len = hdr.frame_length - 1 - cmd_name.len();

        match cmd_name {
            b"READY" => command_ready_meta_data(remaining, data_len, logger),
            b"ERROR" => command_error_reason(remaining, logger),
            b"SUBSCRIBE" => command_subscribe_subscription(remaining, data_len, logger),
            b"CANCEL" => command_cancel_subscription(remaining, data_len, logger),
            b"PING" => command_ping(remaining, data_len, logger),
            b"PONG" => command_pong(remaining, data_len, logger),
            _ => Err(nom::Err::Error(nom::error::make_error(
                input,
                nom::error::ErrorKind::OneOf,
            ))),
        }
    }
}

fn command_ready_meta_data<'a>(
    input: &'a [u8],
    data_len: usize,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Command<'a>> {
    let (input, md) = meta_data(input, data_len, logger)?;

    Ok((input, Command::READY(md)))
}

/// Error command
///
/// error-reason = short-size 0*255VCHAR
fn command_error_reason<'a>(
    input: &'a [u8],
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Command<'a>> {
    let (input, len) = be_u8(input)?;
    let (input, error_txt) = take(len as usize)(input)?;
    trace!(logger, "command_error:";
        o!("length" => len),
        o!("content" => ByteSlice(error_txt)));
    Ok((input, Command::ERROR(ByteSlice(error_txt))))
}

/// Subcribe
///
/// subscription = *OCTET
fn subscription<'a>(
    input: &'a [u8],
    len: usize,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], &'a [u8]> {
    let (input, channel_name) = take(len)(input)?;
    trace!(logger, "subscription:";
        o!("length" => len),
        o!("content" => ByteSlice(channel_name)));
    Ok((input, channel_name))
}

fn command_subscribe_subscription<'a>(
    input: &'a [u8],
    data_len: usize,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Command<'a>> {
    trace!(logger, "command subscribe:"; o!("data_len" => data_len));
    let (input, channel_name) = subscription(input, data_len, logger)?;
    Ok((input, Command::SUBSCRIBE(ByteSlice(channel_name))))
}

fn command_cancel_subscription<'a>(
    input: &'a [u8],
    data_len: usize,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Command<'a>> {
    trace!(logger, "command cancel:"; o!("data_len" => data_len));

    let (input, channel_name) = subscription(input, data_len, logger)?;
    Ok((input, Command::CANCEL(ByteSlice(channel_name))))
}

/// Ping command
/// ping = command-size %d4 "PING" ping-ttl ping-context
/// ping-ttl = 2OCTET
/// ping-context = *OCTET
fn command_ping<'a>(
    input: &'a [u8],
    data_len: usize,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Command<'a>> {
    assert!(data_len > 2);
    let (input, ttl) = be_u16(input)?;
    let (input, context) = take(data_len - 2)(input)?;
    trace!(logger, "command ping:"; o!("ttl" => ttl), o!("context" => ByteSlice(context)));

    Ok((input, Command::PING(Ping { ttl, context })))
}

fn command_pong<'a>(
    input: &'a [u8],
    data_len: usize,
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], Command<'a>> {
    let (input, context) = take(data_len)(input)?;
    trace!(logger, "command pong:"; o!("context" => ByteSlice(context)));

    Ok((input, Command::PONG(Pong { context })))
}
