use crate::world::person::*;
use crate::world::room::*;

/// Messages from, e.g., commands
#[derive(Clone, Debug)]
pub enum Message {
    Arrive {
        id: PersonId,
        name: String,
        loc: RoomId,
    },
    Depart {
        id: PersonId,
        name: String,
        loc: RoomId,
    },
    Say {
        speaker: PersonId,
        speaker_name: String,
        loc: RoomId,
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

    pub fn new_location(&self, _receiver: PersonId) -> Option<RoomId> {
        // TODO return a location on movement when receiver is the given id
        None
    }
}