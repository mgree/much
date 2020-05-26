use std::env;
use std::io;

use tokio::net::TcpListener;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());
    
    let mut listener = TcpListener::bind(&addr).await?;

    println!("much v{} starting up on {}", VERSION, addr);


    println!("much v{} shutting down on {}", VERSION, addr);
    Ok(())
}
