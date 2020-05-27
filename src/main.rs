extern crate much;

use std::env;
use std::error::Error;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::Mutex;

use much::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("much v{}", VERSION);

    let state = Arc::new(Mutex::new(State::new()));
    
    println!("loaded state");

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());
    
    let mut listener = TcpListener::bind(&addr).await?;
    println!("listening on {}", addr);

    serve(state, &mut listener).await?;

    Ok(())
}
