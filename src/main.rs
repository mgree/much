extern crate much;

use std::env;
use std::io;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::Mutex;

use much::*;

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("much v{}", VERSION);

    let state = Arc::new(Mutex::new(State::new()));
    
    println!("loaded state");

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());
    
    let mut listener = TcpListener::bind(&addr).await?;

    println!("listening on {}", addr);

    loop {
        let (stream, addr) = listener.accept().await?;

        let state = Arc::clone(&state);

        // Spawn our handler to be run asynchronously.
        tokio::spawn(async move {
            if let Err(e) = process(state, stream, addr).await {
                println!("an error occurred; error = {:?}", e);
            }
        });
    }

    println!("shutting down on {}", addr);
    Ok(())
}
