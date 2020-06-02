use crate::world::room::*;

/// Unique ID numbers for each person
pub type PersonId = u64;

// Number of characters to use for the password salt
pub const PASSWD_SALT_LENGTH: usize = 16; 

// TODO offer a shorter version of this, w/o password info---use THAT one in memory

/// A person/user. Not necessarily connected.
#[derive(Clone)]
pub struct Person {
    pub id: PersonId,
    pub name: String,
    /// Last known location/default location
    pub loc: RoomId,

    /// The salt for the password (Base64 encoded string of length `PASSWD_SALT_LENGTH`)
    pub salt: String,
    /// The hashed password
    pub password: String,
}