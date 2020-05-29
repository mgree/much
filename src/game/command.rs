use std::error::Error;
use std::fmt;
use std::sync::Arc;

use tokio::sync::{Mutex};

use tracing::{info, span, Level};

use crate::game::message::*;
use crate::game::person::*;
use crate::game::room::*;
use crate::game::state::*;

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
                            loc,
                            text,
                        },
                    )
                    .await
            }
            Command::Shutdown => state.lock().await.shutdown(),
        }
    }
}