use std::error::Error;
use std::fmt;
use std::sync::Arc;

use tokio::sync::{Mutex};

use tracing::{info, span, Level};

use crate::world::message::*;
use crate::world::person::*;
use crate::world::state::*;

#[derive(Clone, Debug)]
pub enum Command {
    Logout,
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
        } else if s == "logout" {
            Ok(Command::Logout)
        } else {
            Ok(Command::Say {
                text: s.to_string(),
            })
        }
    }

    pub fn tag(&self) -> &'static str {
        match self {
            Command::Logout => "logout",
            Command::Say { .. } => "say",
            Command::Shutdown => "shutdown",
        }
    }

    pub async fn run(self, state: Arc<Mutex<State>>, p: &mut Person) {
        let span = span!(Level::INFO, "command", id = p.id);
        let _guard = span.enter();
        info!(command = self.tag());

        match self {
            Command::Logout => state.lock().await.logout(p).await,
            Command::Say { text } => {
                state
                    .lock()
                    .await
                    .roomcast(
                        p.loc,
                        Message::Say {
                            speaker: p.id,
                            speaker_name: p.name.clone(),
                            loc: p.loc,
                            text,
                        },
                    )
                    .await
            }
            Command::Shutdown => state.lock().await.shutdown(),
        }
    }
}