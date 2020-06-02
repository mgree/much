use crate::world::room::*;
use crate::world::state::Connection;

/// Unique ID numbers for each person
pub type PersonId = u64;

// Number of characters to use for the password salt
pub const PASSWD_SALT_LENGTH: usize = 16; 

/// A logged-in connection to the server
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Person {
    pub id: PersonId,
    pub name: String,
    /// Last known location/default location
    pub loc: RoomId,
    pub conn: Connection,
}

impl Person {
    pub fn new(p: &PersonRecord, conn: Connection) -> Self {
        Person {
            id: p.id,
            name: p.name.clone(),
            loc: p.loc,
            conn,
        }
    }
}

/// A person/user. Not necessarily connected.
#[derive(Clone)]
pub struct PersonRecord {
    pub id: PersonId,
    pub name: String,
    /// Last known location/default location
    pub loc: RoomId,

    /// The salt for the password (Base64 encoded string of length `PASSWD_SALT_LENGTH`)
    pub salt: String,
    /// The hashed password
    pub password: String,
}