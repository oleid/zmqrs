#![feature(ptr_wrapping_offset_from)]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

// http://zmtp.org/page:read-the-docs

mod command;
mod frame;
mod greeting;
mod message;

pub use command::{command, Command, Ping, Pong};
pub use frame::{frame, Frame, FrameHeader};
pub use greeting::{greeting, Greeting};
pub use message::{message, Message};
use nom::error::ErrorKind;

extern crate alloc;

#[macro_use]
pub extern crate slog;

#[derive(Debug, Clone)]
pub struct ByteSlice<T>(pub T);

impl From<(&bytes::Bytes, ByteSlice<&[u8]>)> for ByteSlice<bytes::Bytes> {
    fn from(input: (&bytes::Bytes, ByteSlice<&[u8]>)) -> Self {
        let (buffer, subset) = input;
        ByteSlice(buffer.slice_ref(subset.0))
    }
}

#[cfg(feature = "std")]
mod if_std {
    // TODO:
    // Eigenes Parser-Struct entfernen, nur Decoder implementieren;
    // Überlegen welche Typen ich nach außen geben muss.
    // TODO: Überlege, ob es sinnvoll wäre für diese properties-maps
    // die bytes in einer hash-map zu allozieren und diese dann rauszuschicken;
    // für die Daten selbst kann man diese von BytesMut abknabbern, aber für
    // die kleinen Dinger wäre das wohl zu viel Arbeit sie zu zerlegen.

    use crate::prelude::*;
    use bytes::{Buf, Bytes, BytesMut};
    use futures::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt};
    use futures_codec::{Decoder, FramedRead};
    use nom::AsBytes;
    use std::error::Error;

    pub struct Parser {
        logger: slog::Logger,
        _settings: Greeting,
        buffer: BytesMut,
    }

    #[derive(Debug)]
    pub enum ParserError {
        Unspecified,
        IoError(std::io::Error),
    }

    impl<'a> From<nom::Err<(&'a [u8], nom::error::ErrorKind)>> for ParserError {
        fn from(_: nom::Err<(&'a [u8], nom::error::ErrorKind)>) -> Self {
            ParserError::Unspecified
        }
    }

    impl<'a> From<std::io::Error> for ParserError {
        fn from(e: std::io::Error) -> Self {
            ParserError::Unspecified
        }
    }

    impl core::fmt::Display for ParserError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
            match self {
                ParserError::Unspecified => write!(f, "Unspecified error"),
                ParserError::IoError(e) => write!(f, "IoError: {}", e),
            }
        }
    }
    impl std::error::Error for ParserError {}
/*
    impl Parser {
        pub fn new<L: Into<slog::Logger>>(
            initial_data: &[u8],
            logger: L,
        ) -> nom::IResult<&[u8], Self> {
            let mut logger: slog::Logger = logger.into();
            let (input, settings) = greeting(initial_data, &mut logger)?;

            Ok((
                input,
                Parser {
                    logger,
                    _settings: settings,
                    buffer: BytesMut::new(),
                },
            ))
        }

        pub fn process_frame<'a>(
            &'a mut self,
            input: &'a [u8],
        ) -> Result<(&'a [u8], Frame<&'a str, &'a [u8]>), ParserError> {
            frame(input, &mut self.logger).map_err(ParserError::from)
        }

        #[cfg(feature = "std")]
        pub async fn blupp<R>(&mut self, input: &mut R) -> Result<(), ParserError>
        where
            R: AsyncRead + core::marker::Unpin,
        {
            let initial_read_len = 9; // 1 flag byte + up to 8 size byte
            assert!(self.buffer.len() == 0); // check, that nothing is left in buffer.
            self.buffer.resize(initial_read_len, 0);

            input
                .read_exact(&mut self.buffer)
                .await
                .map_err(ParserError::from)?;

            let (remaining, header) =
                frame_header(&self.buffer, &mut self.logger).map_err(ParserError::from)?;
            {
                let consumed = self.buffer.len() - remaining.len();
                self.buffer.advance(consumed);
            }

            Ok(())
        }
    }
*/
    enum Greeting {
        MajorVesion(core::num::NonZeroU8),
        MinorAndSecMechanism(Version, SecurityMechanism),
    }

    struct GreetingParser {
        logger: slog::Logger,
        state: Option<Greeting>,
    }

    fn filter_short_read<'a, V>(res: nom::IResult<&'a [u8], V>) -> Result<Option<(&'a [u8], V)>, ParserError> {
        match res {
            Ok(v) => Ok(Some(v)),
            Err(nom::Err::Incomplete(_)) => return Ok(None), // will try again if more from the buffer is read
            Err(e) => return Err(e.into()),
        }
    }

    impl Decoder for GreetingParser {
        type Item = Frame<Bytes, Bytes>;
        type Error = ParserError;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
           filter_short_read(frame_header(src.as_ref(), &mut self.logger))?
                .and_then(|(pos, hdr)|
            {
                let n_consumed = src.len() - pos.len();

                if hdr.frame_length + n_consumed < src.len() {
                    // will try again if more from the buffer is read
                    Ok(None)
                } else {
                    src.advance(n_consumed);

                    // the actual parsing
                    let frame_bytes = src.split_to(hdr.frame_length).freeze();
                    let (_, parsed_frame) = frame_body(&frame_bytes, &hdr, &mut self.logger)?;
                    let owned_frame = Frame::<Bytes, Bytes>::from((&frame_bytes, parsed_frame));
                    Ok(Some(owned_frame))
                }t
            })


        }
    }

    struct FrameParser {
        logger: slog::Logger,
    }

    impl Decoder for FrameParser {
        type Item = Frame<Bytes, Bytes>;
        type Error = ParserError;

        fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            let (hdr_bytes, hdr) = match frame_header(src.as_ref(), &mut self.logger) {
                Ok((pos, hdr)) => {
                    let n_consumed = src.len() - pos.len();
                    (n_consumed, hdr)
                }
                Err(nom::Err::Incomplete(_)) => return Ok(None), // will try again if more from the buffer is read
                Err(e) => return Err(e.into()),
            };

            if hdr.frame_length + hdr_bytes < src.len() {
                // will try again if more from the buffer is read
                return Ok(None);
            }
            src.advance(hdr_bytes);

            // the actual parsing
            let frame_bytes = src.split_to(hdr.frame_length).freeze();
            let (_, parsed_frame) = frame_body(&frame_bytes, &hdr, &mut self.logger)?;
            let owned_frame = Frame::<Bytes, Bytes>::from((&frame_bytes, parsed_frame));
            Ok(Some(owned_frame))
        }
    }
} // std

pub mod prelude {
    pub use crate::command::*;
    pub use crate::frame::*;
    pub use crate::greeting::*;
    pub use crate::message::*;
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use hex_literal::hex;
    use slog::*;

    pub fn make_logger() -> slog::Logger {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        let drain = LevelFilter::new(drain, Level::Debug).fuse();

        slog::Logger::root(drain, o!())
    }

    #[test]
    fn client_server_chat() {
        let logger = &mut make_logger().new(o!("test" => "client_server_chat"));

        // network capture of client/server chat of hello_world python example.
        // client starts and they talk in turns
        let server_ready = hex!(
            "   04 19 05 52 45 41 44 59  0b 53 6f 63 6b 65 74 2d
                54 79 70 65 00 00 00 03  52 45 50"
        );

        let client_ready_and_data = hex!(
            "   04 26 05 52 45 41 44 59  0b 53 6f 63 6b 65 74 2d
                54 79 70 65 00 00 00 03  52 45 51 08 49 64 65 6e
                74 69 74 79 00 00 00 00  01 00 00 05 48 65 6c 6c
                6f"
        );
        let server_answer = hex!("01 00 00 05 57 6f 72 6c 64");

        frame(&server_ready, logger).unwrap();

        frame(&client_ready_and_data, logger).unwrap();

        frame(&server_answer, logger).unwrap();
    }

    #[test]
    fn test_extract_from_slice() {
        let b = bytes::Bytes::from("Hallo Welt");
        let (_s0, s1) = b.as_ref().split_at(6);

        assert_eq!(s1, b"Welt");
        let b1 = b.slice_ref(s1);

        assert_eq!(b1.as_ref(), b"Welt");
    }
}
