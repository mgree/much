/// Unique ID numbers for each person
pub type PersonId = u64;

/// Someone who is connected
pub struct Person {
    pub id: PersonId,
    pub name: String,
}