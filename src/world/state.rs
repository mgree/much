use std::cmp::{Eq, PartialEq};
use std::collections::HashMap;
use std::net::SocketAddr;

use tokio::sync::{mpsc};

use tracing::{info, trace, warn};

use crate::world::message::*;
use crate::world::person::*;
use crate::world::room::*;

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
/// or statelessly via an HTTP session (possibly in multiple rooms!).
/// 
/// Each such connection will have its own message queue.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Connection {
    /// TCP sessions merely need to track the peer
    TCP { addr: SocketAddr },
    /// Each HTTP session (keyed by the `String` in the cookie) can be in more than one room at a time---and each one needs its own queue
    HTTP { session: String, loc: RoomId },
}

pub type MessageQueueTX = mpsc::UnboundedSender<Message>;
pub type MessageQueueRX = mpsc::UnboundedReceiver<Message>;
