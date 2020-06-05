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
    /// Next `PersonId` to generate
    next_id: PersonId,
    /// Each PersonId is associated with Person data
    people: HashMap<PersonId, PersonRecord>,
    /// Index of names to PersonId
    names: HashMap<String, PersonId>,
    /// Who is in a room
    rooms: HashMap<RoomId, HashSet<Person>>,

    /// CONNECTION INFO
    ///
    /// Each `PersonId` has exactly one connection
    peers: HashMap<PersonId, Connection>, // TODO do we actually need to track this?
    /// Each `PersonId` has a corresponding message queue
    queues: HashMap<PersonId, MessageQueueTX>,
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

    pub fn register_connection(&mut self, id: PersonId, conn: Connection, tx: MessageQueueTX) {
        self.peers.insert(id, conn);
        self.queues.insert(id, tx);
    }

    pub fn unregister_connection(&mut self, id: PersonId) {
        if let None = self.peers.remove(&id) {
            warn!(id, "no connection to unregister");
        }
        if let None = self.queues.remove(&id) {
            warn!(id, "no queue to unregister");
        }
    }

    pub async fn logout(&mut self, p: &Person) {
        self.depart(p).await;

        let conn = match self.peers.remove(&p.id) {
            None => {
                warn!(p.id, "no connection to terminate on logout");
                return ();
            },
            Some(conn) => conn,
        };

        let q = match self.queues.remove(&p.id) {
            None => {
                warn!(p.id, "no connection to terminate on logout");
                return ();
            },
            Some(q) => q,
        };

        if let Connection::TCP { .. } = conn {
            let _ = q.send(Message::Logout);
        }

        // TODO force end of HTTP session?
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

        // find out who's there
        let people = match self.rooms.get(&loc) {
            None => {
                error!(loc, ?message, "room not found in rooms table");
                return ();
            },
            Some(people) => people,
        };

        // let 'em hear about it
        for p in people {
            let q = self.queues.get(&p.id);

            match q {
                None => warn!(
                    loc,
                    ?p,
                    "listed in room, but no message queue... disconnected?"
                ),
                Some(q) => match q.send(message.clone()) {
                    Err(e) => warn!(loc, ?p, ?e, "bad message queue"),
                    Ok(()) => (),
                },
            }
        }
    }

    pub async fn depart(&mut self, p: &Person) {
        // TODO extra parameter indicating where we're going:
        //  - other room (visible)
        //  - logoff (visible)
        //  - private room (invisible)
        info!(?p, "depart");

        let people = match self.rooms.get_mut(&p.loc) {
            None => {
                error!(?p, "not listed in departing room");
                return ();
            },
            Some(people) => people,
        };

        people.remove(p);

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
        }

        let new_room = self.room_mut(loc);
        new_room.insert(p.clone());

        let msg = Message::Arrive {
            id: p.id,
            name: p.name.clone(),
            loc: loc,
        };
        self.roomcast(loc, msg).await;
    }
}

/// A connection to the server, either directly over TCP (e.g., telnet or a MUD client)
/// or statelessly via an HTTP session.
///
/// Each such connection will have its own message queue.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Connection {
    /// TCP sessions merely need to track the peer
    TCP { addr: SocketAddr },
    /// HTTP sessions track the session ID
    HTTP { session: String },
}

pub type MessageQueueTX = mpsc::UnboundedSender<Message>;
pub type MessageQueueRX = mpsc::UnboundedReceiver<Message>;
