extern crate much;

use std::convert::Infallible;
use std::env;
use std::error::Error;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use tracing::{info, Level};
use tracing_subscriber;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};

use much::*;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();

    // TODO parse command-line arguments properly
    let addr = args.nth(1).unwrap_or_else(|| "127.0.0.1".to_string());
    let tcp_port = args.next().unwrap_or_else(|| "4000".to_string());
    let http_port = args.next().unwrap_or_else(|| "4080".to_string());
    let timeout: Option<u64> = args.next().unwrap_or_else(|| "30".to_string()).parse().ok();

    // initialize logging
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // TODO log to a file?
        .with_max_level(Level::INFO)
        .init();

    info!("much v{}", VERSION);

    let state = Arc::new(Mutex::new(State::new()));
    info!("loaded state");

    let tcp_addr = format!("{}:{}", addr, tcp_port);
    let tcp_server = tcp_serve(state.clone(), tcp_addr.clone());
    let http_addr = format!("{}:{}", addr, http_port);
    let http_server = http_serve(state.clone(), http_addr.clone());

    let runtime = tokio::runtime::Runtime::new().unwrap();
    info!("initialized tokio runtime");

    runtime.spawn(tcp_server);
    info!("started TCP server on {}", tcp_addr);

    runtime.spawn(http_server);
    info!("started HTTP server on {}", http_addr);

    if let Some(secs) = timeout {
        info!("shutdown timer: {} seconds", secs);
        runtime.shutdown_timeout(Duration::from_secs(secs));
    }

    info!("shutting down");
    Ok(())
}

async fn http_serve<A: ToSocketAddrs + std::fmt::Display>(
    state: Arc<Mutex<State>>,
    addr_spec: A,
) -> Result<(), Box<dyn Error + Send>> {
    let mut addrs = addr_spec.to_socket_addrs().unwrap();
    let addr = addrs.next().unwrap();
    assert_eq!(
        addrs.next(),
        None,
        "expected a unique bind location for the HTTP server, but {} resolves to at least two",
        addr_spec
    );

    let make_svc = make_service_fn(move |_conn| {
        let state = state.clone();

        async move { Ok::<_, Infallible>(service_fn(move |req| hello_world(state.clone(), req))) }
    });

    let server = Server::bind(&addr).serve(make_svc);
    match server.await {
        Ok(()) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

async fn hello_world(
    _state: Arc<Mutex<State>>,
    _req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(format!("much v{}", VERSION).into()))
}
