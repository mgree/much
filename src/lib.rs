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

/// The global shared state
pub struct State {
    next_id: PersonId,
    /// Each PersonId is associated with Person data
    people: HashMap<PersonId, Person>,
    /// Each `Peer` is associated with a `PersonId`
    peers: HashMap<Connection, PersonId>,
    // Each PersonId has a corresponding message queue
    queues: HashMap<PersonId, MessageQueueTX>,
}

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
type PersonId = u64;

/// Someone who is connected
pub struct Person {
    id: PersonId,
    name: String,
}

// TODO pre-resolve names (currenly resolving once per player!!!); should also drop the state dependency on render
/// Messages from, e.g., commands
#[derive(Clone, Debug)]
pub enum Message {
    Arrive { id: PersonId },
    Depart { id: PersonId },
    Say { speaker: PersonId, text: String },
}

impl Message {
    pub async fn render(&self, state: Arc<Mutex<State>>, receiver: PersonId) -> String {
        // LATER i18n
        match self {
            Message::Arrive { id } if *id == receiver => "".to_string(),
            Message::Arrive { id } => format!("{} arrived.", state.lock().await.person(id).name),
            Message::Depart { id } if *id == receiver => "".to_string(),
            Message::Depart { id } => format!("{} left.", state.lock().await.person(id).name),
            Message::Say { speaker, text } if *speaker == receiver => {
                format!("You say, '{}'", text)
            }
            Message::Say { speaker, text } => format!(
                "{} says, '{}'",
                state.lock().await.person(speaker).name,
                text
            ),
        }
    }
}

impl State {
    pub fn new() -> Self {
        State {
            next_id: 0,
            people: HashMap::new(),
            peers: HashMap::new(),
            queues: HashMap::new(),
        }
    }

    pub fn fresh_id(&mut self) -> PersonId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn new_person(&mut self, name: &str) -> PersonId {
        let id = self.fresh_id();

        let name = name.to_string();
        let person = Person { id, name };

        self.people.insert(id, person);

        id
    }

    pub fn person(&self, id: &PersonId) -> &Person {
        assert!(self.people.contains_key(&id));
        self.people.get(&id).unwrap()
    }

    pub fn register_tcp_connection(&mut self, id: PersonId, addr: SocketAddr, tx: MessageQueueTX) {
        self.peers.insert(Connection::TCP { addr }, id);
        self.queues.insert(id, tx);
    }

    pub fn unregister_tcp_connection(&mut self, addr: SocketAddr) {
        let addr = Connection::TCP { addr };
        assert!(self.peers.contains_key(&addr));

        self.peers.remove(&addr);
    }

    /// Send a message to _all_ peers.
    pub async fn broadcast(&mut self, message: Message) {
        for p in self.queues.iter_mut() {
            let _ = p.1.send(message.clone());
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
        let id = state.lock().await.new_person(&name);

        state.lock().await.register_tcp_connection(id, addr, tx);

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
                let msg = Message::Say {
                    speaker: peer.id,
                    text: msg,
                };
                let mut state = state.lock().await;
                state.broadcast(msg).await;
            }

            Ok(PeerMessage::SendToPeer(msg)) => {
                let s = msg.render(state.clone(), peer.id).await;
                peer.lines.send(s).await?;
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

        // actually log them off
        state.unregister_tcp_connection(addr);

        // announce it to everyone
        let msg = Message::Depart { id: peer.id };
        println!("{:?}", msg);
        state.broadcast(msg).await;
    }

    Ok(())
}
