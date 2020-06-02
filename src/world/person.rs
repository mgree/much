use crate::world::room::*;

/// Unique ID numbers for each person
pub type PersonId = u64;

// Number of characters to use for the password salt
pub const PASSWD_SALT_LENGTH: usize = 16; 

/// A person/user. Not necessarily connected.
#[derive(Clone)]
pub struct Person {
    pub id: PersonId,
    pub name: String,
    /// Last known location/default location
    pub loc: RoomId,

    /// The salt for the password (of length `PASSWD_SALT_LENGTH`)
    pub salt: String,
    /// The hashed password
    pub password: String,
}