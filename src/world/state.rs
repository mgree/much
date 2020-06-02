use std::cmp::{Eq, PartialEq};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use rand::RngCore;

use tokio::sync::mpsc;

use tracing::{error, info, trace, warn};

use crate::world::message::*;
use crate::world::person::*;
use crate::world::room::*;

/// The global shared state
pub struct State {
    /// CONFIGURATION
    /// 
    /// Password hashing configuration
    password_config: argon2::Config<'static>,

    /// DATABASE
    /// 
    /// Next person ID to generate
    next_id: PersonId,
    /// Each PersonId is associated with Person data
    people: HashMap<PersonId, PersonRecord>,
    /// Index of names to PersonId
    names: HashMap<String, PersonId>,
    /// Who is in a room
    rooms: HashMap<RoomId, HashSet<Person>>,

    /// CONNECTION INFO
    ///
    /// Each `PersonId` has some number of connections
    peers: HashMap<PersonId, HashSet<Connection>>,
    /// Each Connection has a corresponding message queue
    queues: HashMap<Connection, MessageQueueTX>,
}

impl State {
    pub fn new() -> Self {
        let mut rooms = HashMap::new();
        rooms.insert(INITIAL_LOC, HashSet::new());

        State {
            next_id: 0,
            people: HashMap::new(),
            names: HashMap::new(),
            rooms,
            peers: HashMap::new(),
            queues: HashMap::new(),
            password_config: argon2::Config::default(),
        }
    }

    pub fn shutdown(&mut self) {
        warn!("shutdown initiated");
        // TODO coordinate with top-level tokio runtime via tokio::sync::oneshot
        std::process::exit(0);
    }

    pub fn fresh_id(&mut self) -> PersonId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn new_person(&mut self, name: &str, password: &str) -> PersonRecord {
        let id = self.fresh_id();
        info!(id = id, name = name, "registered");

        // TODO this is a race :(
        // if someone registers a name while someone else is mid-registration, we'll fail this check :(
        // best solution: return a result here and handle the race up above
        assert!(!self.names.contains_key(name));
        let name = name.to_string();
        self.names.insert(name.clone(), id);

        let mut salt: [u8; PASSWD_SALT_LENGTH / 4] = [0; PASSWD_SALT_LENGTH / 4];
        rand::thread_rng().fill_bytes(&mut salt);
        let salt = base64::encode(salt);

        // TODO handle error case
        let password =
            argon2::hash_encoded(password.as_bytes(), salt.as_bytes(), &self.password_config)
                .unwrap();

        let person = PersonRecord {
            id,
            loc: INITIAL_LOC,
            name,
            salt,
            password,
        };

        self.people.insert(id, person.clone());

        person
    }

    pub fn room(&self, loc: RoomId) -> &HashSet<Person> {
        self.rooms.get(&loc).expect("room should exist")
    }

    pub fn room_mut(&mut self, loc: RoomId) -> &mut HashSet<Person> {
        self.rooms.get_mut(&loc).expect("room should exist")
    }

    pub fn person(&self, id: &PersonId) -> &PersonRecord {
        assert!(self.people.contains_key(&id));
        self.people.get(&id).unwrap()
    }

    pub fn person_by_name(&self, name: &str) -> Option<PersonRecord> {
        let id = self.names.get(name)?;
        self.people.get(id).map(|p| p.clone()).or_else(|| {
            error!(name, id, "in names but not people");
            None
        })
    }

    pub fn register_tcp_connection(&mut self, id: PersonId, addr: SocketAddr, tx: MessageQueueTX) {
        let conn = Connection::TCP { addr };

        match self.peers.get_mut(&id) {
            Some(conns) => { 
                let _ = conns.insert(conn.clone());
            },
            None => { 
                let mut conns = HashSet::new();
                conns.insert(conn.clone());
                self.peers.insert(id, conns);
            },
        };
        self.queues.insert(conn, tx);
    }

    pub fn unregister_tcp_connection(&mut self, id: PersonId, addr: SocketAddr) {
        let conn = Connection::TCP { addr };

        assert!(self.peers.contains_key(&id));
        assert!(self.queues.contains_key(&conn));

        let conns = self.peers.get_mut(&id).unwrap();
        assert!(conns.contains(&conn));

        conns.remove(&conn);

        if conns.is_empty() {
            info!(id=id, "last connection, dropping queues");
            self.queues.remove(&conn);
        }
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
        let room_conns = self.peers.values().flatten();

        for conn in room_conns {
            let p = self.queues.get(&conn);

            match p {
                None => warn!(
                    loc,
                    ?conn, "listed in room, but no message queue... disconnected?"
                ),
                Some(p) => match p.send(message.clone()) {
                    Err(e) => warn!(loc, ?conn, ?e, "bad message queue"),
                    Ok(()) => (),
                },
            }
        }
    }

    pub async fn depart(&mut self, p: &Person) {
        info!(?p, "depart");

        let room = self.rooms.get_mut(&p.loc).unwrap();

        room.remove(p);

        let msg = Message::Depart {
            id: p.id,
            name: p.name.clone(),
            loc: p.loc,
        };
        self.roomcast(p.loc, msg).await;
    }

    pub async fn arrive(&mut self, p: &mut Person, loc: RoomId) {
        info!(?p, "arrive");

        if p.loc != loc {
            let old_room = self.room_mut(p.loc);
            old_room.remove(p);

            p.loc = loc;
            let new_room = self.room_mut(loc);

            new_room.insert(p.clone());    
        }

        let msg = Message::Arrive {
            id: p.id,
            name: p.name.clone(),
            loc: loc,
        };
        self.roomcast(loc, msg).await;
    }
}

/// A connection to the server, either directly over TCP (e.g., telnet or a MUD client)
/// or statelessly via an HTTP session (possibly in multiple rooms!).
///
/// Each such connection will have its own message queue.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Connection {
    /// TCP sessions merely need to track the peer
    TCP { addr: SocketAddr },
    /// Each HTTP session (keyed by the `String` in the cookie) can be in more than one room at a time---and each one needs its own queue
    HTTP { session: String, loc: RoomId },
}

pub type MessageQueueTX = mpsc::UnboundedSender<Message>;
pub type MessageQueueRX = mpsc::UnboundedReceiver<Message>;
