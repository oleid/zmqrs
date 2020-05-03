#![forbid(unsafe_code)]

// General idea:
// Model different states via struct.
// Model state transitions via From trait
// Model errors via Result and some transition error
//
// Open questions
//
// * How to get data in?
// * What are all the states?
// *

extern crate alloc;

use alloc::vec::Vec;
use slog::Logger;
use zmqrs_parser::Frame;

//use state_machine_future::RentToOwn;

pub struct Protocol {
    logger: Logger,
}
/*
async fn zmq_protocol(input: ZmqInStream, output: ZmqOutStream) -> Result<(), ProtocolError>
{
    let mut state  = ProtocolState::Init;
    while let msg = input.get() {
        match state {
            ProtocolState::Init =>
                send_version_pt1(&mut output).await?,
            ProtocolState::
        }
    }
        unimplemented!()
}*/

//// TODO: Think about how useful the ingo data and the outgo data should be brought under one roof.
//// Possibly by further state. If sent, maybe something lie waitingforanswer

struct VersionExchange1 {
    my_version_sent: bool,
    other_major_version: Option<core::num::NonZeroU8>,
}

struct VersionExchange2 {
    my_greeting_finished: bool,
    other_greeting: Option<zmqrs_parser::Greeting>,
}

struct SecurityHandshake {
    // TODO
}

struct MetaDataExchange {
    my_metadata_sent: bool,
    //other_metadata: Option<zmqrs_parser::MetaData>
}
enum ProtocolError {}

enum ProtocolState {
    /// Before any data exchange.
    Init,
    /// The client sends a partial greeting (11 octets) greeting to the server, and at the same time
    /// (before receiving anything from the client), the server also sends a partial greeting.
    VersionExchange1(VersionExchange1),
    /// The client and server read the major version number (%x03) and send the rest of their
    /// greeting to each other.
    VersionExchange2(VersionExchange2),

    /// The client and server now perform the secutity handshake.
    /// Depending on the security mechanism, there might be _internal_ states.
    SecurityHandshake(SecurityHandshake),

    /// The client sends a frame with connection metadata, i.e. SocketType
    /// The server validates the socket type, accepts it
    MetaDataExchange(MetaDataExchange),

    /// The connection is agreed uppon. Now data is exchanged.
    WaitingForCommandOrMessage,

    /// Inoperable state - i.e. both sides did not agree on the connection.
    Inoperable(ProtocolError),
}

/*
trait StateChange<To>
{
    fn change<'a>(&self, frame: &Frame<'a>) -> To;
}

mod connection_stages
{
    struct Start;

    struct SecurityHandshake;

    struct MetadataExchange;

    struct SetupDone; //  go from here to data_exchange
}

mod version_negotiaton
{
    // TODO: partial reads/writes to allow downgrade.
}

mod security_negotiation
{
    // TODO: think about anything beyond NULL
}

mod data_exchange
{
    pub struct WaitForCommandOrMessage;

    pub struct PartialMessage;

    pub struct PartialCommand;
}

impl StateChange<data_exchange::WaitForCommandMessage> for data_exchange::WaitForCommandMessage
{
    fn change<'a>(&self, frame: &Frame<'a>) -> data_exchange::WaitForCommandMessage {
        unimplemented!()
    }
}

enum ProtocolError {}

impl Protocol {
    pub async fn blupp<R>(&mut self, input: &mut R) -> Result<(), std::io::Error>
    where
        R: AsyncRead + core::marker::Unpin,
    {
        let initial_read_len = 9; // 1 flag byte + up to 8 size byte
        assert!(self.buffer.len() == 0); // check, that nothing is left in buffer.
        self.buffer.resize(initial_read_len, 0);

        input.read_exact(&mut self.buffer).await?;

        let (remaining, header) =
            frame_header(&self.buffer, &mut self.logger).map_err(convert_err)?;
        {
            let consumed = self.buffer.len() - remaining.len();
            self.buffer.advance(consumed);
        }

        Ok(())
    }
}
*/
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
