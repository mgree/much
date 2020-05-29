#![allow(dead_code)]

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

use tracing::{error, info, span, trace, Level};

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

mod game;

use game::command::*;
use game::message::*;
use game::person::*;
use game::room::*;
use game::state::*;

pub type GameState = Arc<Mutex<State>>;

pub fn init() -> GameState {
    Arc::new(Mutex::new(State::new()))
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
        state: GameState,
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
    _state: GameState,
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
                let loc = DUMMY_ROOM_ID;

                return Ok((name.to_string(), loc));
            }
            _ => return Err(Box::new(LoginAbortedError { addr })),
        }
    }
}

pub async fn process(
    state: GameState,
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
            loc: peer.loc,
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
                if let Some(loc) = msg.new_location(peer.id) {
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
            loc: peer.loc,
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
