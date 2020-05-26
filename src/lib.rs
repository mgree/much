#![allow(dead_code)]

use std::cmp::{Eq, PartialEq};
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::SinkExt;
use tokio::net::TcpStream;
use tokio::stream::{Stream, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

/// Someone who is connected to the server, either directly over TCP (e.g., telnet or a MUD client)
/// or statelessly via an HTTP session
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Connection {
    TCP { addr: SocketAddr },
    HTTP { session: String },
}

type MessageQueueTX = mpsc::UnboundedSender<Message>;
type MessageQueueRX = mpsc::UnboundedReceiver<Message>;

/// Unique ID numbers for each person
// TODO fresh generation, database of all known
type PersonId = u32;

/// Someone who is connected
pub struct Person {
    id: PersonId,
    name: String,
    /// Pending messages for this client
    tx: MessageQueueTX,
}

/// Messages from, e.g., commands
#[derive(Clone, Debug)]
pub enum Message {
    Arrive { id: PersonId },
    Depart { id: PersonId },
    Say { speaker: PersonId, text: String },
}

impl Message {
    pub fn render(&self, receiver: PersonId) -> String {
        match self {
            Message::Arrive { id } => format!("{} arrived.", id), // TODO resolve name
            Message::Depart { id } => format!("{} left.", id), // TODO resolve name
            Message::Say { speaker, text } => format!("{} says, '{}'", speaker, text),
        }
    }
}

/// The global shared state
pub struct State {
    next_id: PersonId,
    /// Each `Peer` is associated with a `Person` record
    peers: HashMap<Connection, Person>
    // TODO cleaner mapping: use PersonId everywhere, then indirect through State
}

impl State {
    pub fn new() -> Self {
        State {
            next_id: 0,
            peers: HashMap::new(),
        }
    }

    pub fn fresh_id(&mut self) -> PersonId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Send a message to _all_ peers.
    pub async fn broadcast(&mut self, message: Message) {
        for p in self.peers.iter_mut() {
            let _ = p.1.tx.send(message.clone());
        }
    }
}

/// Internal messages for managing a peer's `MessageQueue`
#[derive(Clone, Debug)]
enum PeerMessage {
    LineFromPeer(String),
    SendToPeer(Message),
}

struct TCPPeer {
    /// Line-oriented TCP socket (poor-man's telnet)
    ///     
    /// This is the actual place we read from!
    // TODO support IAC codes, MCCP, etc.
    lines: Framed<TcpStream, LinesCodec>,
    /// Who this peer resolves to
    id: PersonId,
    /// Receive-end of the message queue for this connection
    rx: MessageQueueRX,
}

impl TCPPeer {
    async fn new(
        state: Arc<Mutex<State>>,
        lines: Framed<TcpStream, LinesCodec>,
        name: String,
    ) -> io::Result<Self> {
        let addr = lines.get_ref().peer_addr()?;

        let (tx, rx) = mpsc::unbounded_channel();

        // TODO login?! use existing id?
        let id = state.lock().await.fresh_id();

        let person = Person { id, name, tx };

        state
            .lock()
            .await
            .peers
            .insert(Connection::TCP { addr }, person);

        Ok(TCPPeer { lines, id, rx })
    }
}

impl Stream for TCPPeer {
    type Item = Result<PeerMessage, LinesCodecError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // send pending messages to the peer
        if let Poll::Ready(Some(v)) = Pin::new(&mut self.rx).poll_next(cx) {
            return Poll::Ready(Some(Ok(PeerMessage::SendToPeer(v))));
        }

        // connection-dependent read from the peer
        let result: Option<_> = futures::ready!(Pin::new(&mut self.lines).poll_next(cx));

        Poll::Ready(match result {
            Some(Ok(message)) => Some(Ok(PeerMessage::LineFromPeer(message))),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        })
    }
}

pub async fn process(
    state: Arc<Mutex<State>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let mut lines = Framed::new(stream, LinesCodec::new());

    // TODO welcome header, instructions, etc.

    lines
        .send("What is your name, Twitter handle, or email address? ")
        .await?;

    let name = match lines.next().await {
        Some(Ok(line)) => line, // TODO trim, check up
        _ => {
            println!("Couldn't get username from {}; connection reset.", addr);
            return Ok(());
        }
    };

    let mut peer = TCPPeer::new(state.clone(), lines, name.clone()).await?;

    {
        let mut state = state.lock().await;
        let msg = Message::Arrive { id: peer.id };
        println!("{:?}", msg);
        state.broadcast(msg).await;
    }

    while let Some(result) = peer.next().await {
        match result {
            Ok(PeerMessage::LineFromPeer(msg)) => {
                // TODO parse commands here
                let msg = Message::Say { speaker: peer.id,  text: msg };
                let mut state = state.lock().await;
                state.broadcast(msg).await;
            }

            Ok(PeerMessage::SendToPeer(msg)) => {
                peer.lines.send(msg.render(peer.id)).await?;
            }
            Err(e) => {
                println!(
                    "an error occurred while processing messages for {}; error = {:?}",
                    name, e
                );
            }
        }
    }

    {
        let mut state = state.lock().await;
        let msg = Message::Depart { id: peer.id };
        println!("{:?}", msg);
        state.broadcast(msg).await;
    }

    Ok(())
}
