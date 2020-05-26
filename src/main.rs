use std::env;
use std::io;

use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());
    
    let mut listener = TcpListener::bind(&addr).await?;

    Ok(())
}
