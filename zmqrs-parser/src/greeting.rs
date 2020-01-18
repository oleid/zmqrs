use core::convert::TryFrom;
use nom::{
    bytes::complete::{tag, take},
    number::complete::be_u8,
    IResult,
};
use slog::{Error, Record, Serializer};

#[derive(Debug, Clone, PartialEq)]
pub struct Greeting {
    pub version: Version,
    pub mechanism: SecurityMechanism,
    pub as_server: bool,
}

impl slog::Value for Greeting {
    fn serialize(
        &self,
        _record: &Record,
        _key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> Result<(), Error> {
        serializer.emit_u8("ver_major", self.version.major)?;
        serializer.emit_u8("ver_minor", self.version.minor)?;
        serializer.emit_str("mechanism", self.mechanism.description())?;
        serializer.emit_bool("as_server", self.as_server)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
}

impl slog::Value for Version {
    fn serialize(
        &self,
        _record: &Record,
        _key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> Result<(), Error> {
        serializer.emit_u8("major", self.major)?;
        serializer.emit_u8("minor", self.minor)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SecurityMechanism {
    NULL,
    PLAIN,
    CURVE,
}

impl SecurityMechanism {
    fn description(&self) -> &'static str {
        match self {
            SecurityMechanism::NULL => "SecurityMechanism::NULL",
            SecurityMechanism::PLAIN => "SecurityMechanism::PLAIN",
            SecurityMechanism::CURVE => "SecurityMechanism::CURVE",
        }
    }
}

impl slog::Value for SecurityMechanism {
    fn serialize(
        &self,
        _record: &Record,
        _key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> Result<(), Error> {
        serializer.emit_str("mechanism", self.description())
    }
}

impl<'a> TryFrom<&'a [u8]> for SecurityMechanism {
    type Error = nom::Err<(&'a [u8], nom::error::ErrorKind)>;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        // please note: all branches need to have the same length; data is zero-padded
        match &value[0..5] {
            b"NULL\0" => Ok(SecurityMechanism::NULL),
            b"PLAIN" => Ok(SecurityMechanism::PLAIN),
            b"CURVE" => Ok(SecurityMechanism::CURVE),
            _ => Err(nom::Err::Error(nom::error::make_error(
                value,
                nom::error::ErrorKind::Eof,
            ))),
        }
    }
}

/// Greeting of an ZMTP request
///
/// greeting = signature version mechanism as-server filler
pub fn greeting<'a>(input: &'a [u8], logger: &mut slog::Logger) -> IResult<&'a [u8], Greeting> {
    // ;   The greeting announces the protocol details
    // greeting = signature version mechanism as-server filler

    let (input, _) = signature(input, logger)?;
    let (input, version) = version(input, logger)?;
    let (input, sec_mechanism) = mechanism(input, logger)?;
    let (input, as_server) = as_server(input, logger)?;
    let (input, _) = filler(input, logger)?;

    trace!(logger, "greeting valid:";
      o!("version" => &version), o!("mechanism" => &sec_mechanism), o!("as_server" => as_server));

    Ok((
        input,
        Greeting {
            version,
            mechanism: sec_mechanism,
            as_server,
        },
    ))
}

/// Returns ok, if the correct signature is used.
fn signature<'a>(input: &'a [u8], logger: &mut slog::Logger) -> IResult<&'a [u8], ()> {
    // signature = %xFF padding %x7F
    // padding = 8OCTET        ; Not significant

    let (input, _) = tag([0xff])(input)?;
    let (input, _) = take(8u8)(input)?;
    let (input, _) = tag([0x7f])(input)?;
    trace!(logger, "signature valid");

    Ok((input, ()))
}

fn version<'a>(input: &'a [u8], logger: &mut slog::Logger) -> IResult<&'a [u8], Version> {
    // version = version-major version-minor
    // version-major = %x03
    // version-minor = %x01

    let (input, major) = be_u8(input)?;
    let (input, minor) = be_u8(input)?;

    trace!(logger, "version:"; o!("major" => major), o!("minor" => minor));

    Ok((input, Version { major, minor }))
}

fn mechanism<'a>(
    input: &'a [u8],
    logger: &mut slog::Logger,
) -> IResult<&'a [u8], SecurityMechanism> {
    // ;   The mechanism is a null padded string
    // mechanism = 20mechanism-char
    // mechanism-char = "A"-"Z" | DIGIT
    //      | "-" | "_" | "." | "+" | %x0

    let (input, mechanism_str) = take(20u8)(input)?;
    let mec = SecurityMechanism::try_from(mechanism_str)?;

    trace!(logger, "mechanism:"; o!("val" => &mec));

    Ok((input, mec))
}

fn as_server<'a>(input: &'a [u8], logger: &mut slog::Logger) -> IResult<&'a [u8], bool> {
    // ;   Is the peer acting as server for security handshake?
    // as-server = %x00 | %x01

    let (input, val) = be_u8(input)?;

    trace!(logger, "as_server:"; o!("val" => val));

    Ok((input, val == 0x01))
}

fn filler<'a>(input: &'a [u8], logger: &mut slog::Logger) -> IResult<&'a [u8], ()> {
    // ;   The filler extends the greeting to 64 octets
    // filler = 31%x00             ; 31 zero octets
    let (input, _) = tag([0u8; 31])(input)?;
    trace!(logger, "filler:");

    Ok((input, ()))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use hex_literal::hex;
    use slog::*;

    use crate::tests::make_logger;

    #[test]
    fn test_greeting() {
        let mut logger = make_logger().new(o!("test" => "test_greeting"));
        info!(logger, "Starting test_greeting");

        let intro = hex!(
            "   ff 00 00 00 00 00 00 00  01 7f 03 00 4e 55 4c 4c
                00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00
                00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00
                00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00"
        );
        let end_of_intro = &intro[intro.len()..intro.len()];
        assert_eq!(
            greeting(&intro, &mut logger),
            Ok((
                end_of_intro,
                Greeting {
                    version: Version { major: 3, minor: 0 },
                    mechanism: SecurityMechanism::NULL,
                    as_server: false
                }
            ))
        )
    }
}
