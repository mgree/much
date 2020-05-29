#![allow(dead_code)]

use std::cmp::{Eq, PartialEq};
use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};


use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};

use futures::SinkExt;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::stream::{Stream, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

use tracing::{error, info, span, trace, warn, Level};

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
        info!(id = id, name = name, "registered");

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

    /// Send a message to everyone in a given location
    pub async fn roomcast(&mut self, loc: RoomId, message: Message) {
        trace!(loc, message = ?message, "roomcast");

        // TODO look up only those person ids in loc
        let ids_in_room = self.queues.keys();
        for id in ids_in_room {
            let p = self.queues.get(id);

            match p {
                None => warn!(
                    loc,
                    id, "listed in room, but no message queue... disconnected?"
                ),
                Some(p) => match p.send(message.clone()) {
                    Err(e) => warn!(loc, id, ?e, "bad message queue"),
                    Ok(()) => (),
                },
            }
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
type PersonId = u64;

/// Someone who is connected
pub struct Person {
    id: PersonId,
    name: String,
}

/// Unique ID numbers for each room
type RoomId = u64;

const DUMMY_ROOM_ID: RoomId = 4747;

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

    pub fn new_location(&self) -> Option<RoomId> {
        // TODO actually read new location
        None
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

    pub async fn run(self, state: Arc<Mutex<State>>, loc: RoomId, id: PersonId, name: &str) {
        let span = span!(Level::INFO, "command", id = id);
        let _guard = span.enter();
        info!(command = self.tag());

        match self {
            Command::Say { text } => {
                state
                    .lock()
                    .await
                    .roomcast(
                        loc,
                        Message::Say {
                            speaker: id,
                            speaker_name: name.to_string(),
                            text,
                        },
                    )
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
    /// Their locaation (cached, for convenience)
    loc: RoomId,
    /// Receive-end of the message queue for this connection
    rx: MessageQueueRX,
}

impl TCPPeer {
    async fn new(
        state: Arc<Mutex<State>>,
        lines: Framed<TcpStream, LinesCodec>,
        loc: RoomId,
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
            loc,
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
) -> Result<(String, RoomId), Box<dyn Error>> {
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

                // TODO look up location

                return Ok((name.to_string(), DUMMY_ROOM_ID));
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

    let (name, loc) = login(state.clone(), &mut lines, addr).await?;
    let mut peer = TCPPeer::new(state.clone(), lines, loc, name.clone()).await?;

    let span = span!(Level::INFO, "session");
    let _guard = span.enter();
    info!(peer.id, "login");

    {
        let mut state = state.lock().await;
        let msg = Message::Arrive {
            id: peer.id,
            name: peer.name.clone(),
        };
        state.roomcast(loc, msg).await;
    }

    while let Some(result) = peer.next().await {
        match result {
            Ok(PeerMessage::LineFromPeer(msg)) => {
                let cmd = Command::parse(msg)?;

                cmd.run(state.clone(), peer.loc, peer.id, &peer.name).await;
            }

            Ok(PeerMessage::SendToPeer(msg)) => {
                if let Some(loc) = msg.new_location() {
                    peer.loc = loc;
                }
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
        state.roomcast(loc, msg).await;
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

////////////////////////////////////////////////////////////////////////////////
// HTTP STUFF
////////////////////////////////////////////////////////////////////////////////

pub async fn http_serve<A: std::net::ToSocketAddrs + std::fmt::Display>(
    state: Arc<Mutex<State>>,
    addr_spec: A,
) -> Result<(), Box<dyn Error + Send>> {
    let mut addrs = addr_spec.to_socket_addrs().unwrap();
    let addr = addrs.next().unwrap();
    assert_eq!(
        addrs.next(),
        None,
        "expected a unique bind location for the HTTP server, but {} resolves to at least two",
        addr_spec
    );

    let make_svc = make_service_fn(move |_conn| {
        let state = state.clone();

        async move { Ok::<_, Infallible>(service_fn(move |req| hello_world(state.clone(), req))) }
    });

    let server = Server::bind(&addr).serve(make_svc);
    match server.await {
        Ok(()) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

async fn hello_world(
    _state: Arc<Mutex<State>>,
    _req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(format!("much v{}", VERSION).into()))
}