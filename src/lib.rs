#![allow(dead_code)]

use std::cmp::{Eq, PartialEq};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::SinkExt;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::stream::{Stream, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

use tracing::{info, span, error, warn, trace, Level};

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

impl State {
    pub fn new() -> Self {
        State {
            next_id: 0,
            people: HashMap::new(),
            peers: HashMap::new(),
            queues: HashMap::new(),
        }
    }

    pub fn shutdown(&mut self) {
        warn!("shutdown initiated");
        std::process::exit(0);
    }

    pub fn fresh_id(&mut self) -> PersonId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn new_person(&mut self, name: &str) -> PersonId {
        let id = self.fresh_id();
        info!(id=id, name=name, "registered");

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
        trace!(message = ?message, "broadcast");

        for p in self.queues.iter_mut() {
            let _ = p.1.send(message.clone());
        }
    }
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
    Arrive {
        id: PersonId,
        name: String,
    },
    Depart {
        id: PersonId,
        name: String,
    },
    Say {
        speaker: PersonId,
        speaker_name: String,
        text: String,
    },
}

impl Message {
    pub async fn render(&self, receiver: PersonId) -> String {
        // LATER i18n
        match self {
            Message::Arrive { id, .. } if *id == receiver => "".to_string(),
            Message::Arrive { name, .. } => format!("{} arrived.", name),
            Message::Depart { id, .. } if *id == receiver => "".to_string(),
            Message::Depart { name, .. } => format!("{} left.", name),
            Message::Say { speaker, text, .. } if *speaker == receiver => {
                format!("You say, '{}'", text)
            }
            Message::Say {
                speaker_name, text, ..
            } => format!("{} says, '{}'", speaker_name, text),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    Say { text: String },
    Shutdown,
}

#[derive(Debug)]
pub struct ParserError {
    msg: String,
}

impl Error for ParserError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Parse error: {} is not a valid command.", self.msg)
    }
}

impl Command {
    pub fn parse(s: String) -> Result<Command, Box<dyn Error>> {
        let s = s.trim();

        if s == "shutdown" {
            Ok(Command::Shutdown)
        } else {
            Ok(Command::Say {
                text: s.to_string(),
            })
        }
    }

    pub fn tag(&self) -> &'static str {
        match self {
            Command::Say { .. } => "say",
            Command::Shutdown => "shutdown",
        }
    }

    pub async fn run(self, state: Arc<Mutex<State>>, id: PersonId, name: &str) {
        let span = span!(Level::INFO, "command", id = id);
        let _guard = span.enter();
        info!(command = self.tag());

        match self {
            Command::Say { text } => {
                state
                    .lock()
                    .await
                    .broadcast(Message::Say {
                        speaker: id,
                        speaker_name: name.to_string(),
                        text,
                    })
                    .await
            }
            Command::Shutdown => state.lock().await.shutdown(),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// TCP STUFF
////////////////////////////////////////////////////////////////////////////////

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
    /// Their name (cached, for convenience)
    name: String,
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

        Ok(TCPPeer {
            lines,
            id,
            name,
            rx,
        })
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

#[derive(Debug)]
struct LoginAbortedError {
    addr: SocketAddr,
}

impl Error for LoginAbortedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for LoginAbortedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Couldn't get username from {}; connection reset.",
            self.addr
        )
    }
}

pub async fn login(
    _state: Arc<Mutex<State>>,
    lines: &mut Framed<TcpStream, LinesCodec>,
    addr: SocketAddr,
) -> Result<String, Box<dyn Error>> {
    // TODO welcome header, instructions, etc.

    loop {
        lines
            .send("What is your email address or Twitter handle? ")
            .await?;

        match lines.next().await {
            Some(Ok(line)) => {
                let name = line.trim();

                if name.is_empty() || !name.contains('@') {
                    lines
                        .send("Please enter a valid email address or Twitter handle.")
                        .await?;
                    continue;
                }

                return Ok(name.to_string());
            }
            _ => return Err(Box::new(LoginAbortedError { addr })),
        }
    }
}

pub async fn process(
    state: Arc<Mutex<State>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let mut lines = Framed::new(stream, LinesCodec::new());

    let name = login(state.clone(), &mut lines, addr).await?;
    let mut peer = TCPPeer::new(state.clone(), lines, name.clone()).await?;

    let span = span!(Level::INFO, "session");
    let _guard = span.enter();
    info!(peer.id, "login");

    {
        let mut state = state.lock().await;
        let msg = Message::Arrive {
            id: peer.id,
            name: peer.name.clone(),
        };
        state.broadcast(msg).await;
    }

    while let Some(result) = peer.next().await {
        match result {
            Ok(PeerMessage::LineFromPeer(msg)) => {
                let cmd = Command::parse(msg)?;

                cmd.run(state.clone(), peer.id, &peer.name).await;
            }

            Ok(PeerMessage::SendToPeer(msg)) => {
                let s = msg.render(peer.id).await;
                peer.lines.send(s).await?;
            }

            Err(e) => {
                error!(?e, id = peer.id);
            }
        }
    }

    {
        let mut state = state.lock().await;

        // actually log them off
        state.unregister_tcp_connection(addr);

        // announce it to everyone
        let msg = Message::Depart {
            id: peer.id,
            name: peer.name.clone(),
        };
        info!(id = peer.id, "logout");
        state.broadcast(msg).await;
    }

    trace!("disconnected");
    Ok(())
}

pub async fn tcp_serve<A: ToSocketAddrs>(state: Arc<Mutex<State>>, addr: A) -> io::Result<()> {
    let mut listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, addr) = listener.accept().await?;

        let span = span!(Level::INFO, "TCP connection");
        let _guard = span.enter();
        info!(?addr, "connected");

        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = process(state, stream, addr).await {
                error!(?e);
            }
        });
    }
}