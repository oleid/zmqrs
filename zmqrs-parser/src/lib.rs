#![no_std]
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

extern crate alloc;

#[macro_use]
pub extern crate slog;

#[derive(Debug)]
pub struct ByteSlice<'a>(&'a [u8]);

pub struct Parser {
    logger: slog::Logger,
    _settings: Greeting,
}

impl Parser {
    pub fn new<L: Into<slog::Logger>>(initial_data: &[u8], logger: L) -> nom::IResult<&[u8], Self> {
        let mut logger: slog::Logger = logger.into();
        let (input, settings) = greeting(initial_data, &mut logger)?;

        Ok((
            input,
            Parser {
                logger,
                _settings: settings,
            },
        ))
    }

    pub fn process_frame<'a>(&'a mut self, input: &'a [u8]) -> nom::IResult<&'a [u8], Frame> {
        frame(input, &mut self.logger)
    }
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
}
