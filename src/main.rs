extern crate much;

use std::convert::Infallible;
use std::env;
use std::error::Error;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};

use much::*;

fn main() -> Result<(), Box<dyn Error>> {
    println!("much v{}", VERSION);

    let state = Arc::new(Mutex::new(State::new()));
    println!("loaded state");

    let mut args = env::args();

    // TODO parse command-line arguments properly
    let addr = args.nth(1).unwrap_or_else(|| "127.0.0.1".to_string());
    let tcp_port = args.next().unwrap_or_else(|| "4000".to_string());
    let http_port = args.next().unwrap_or_else(|| "4080".to_string());
    let timeout: Option<u64> = args.next().unwrap_or_else(|| "30".to_string()).parse().ok();

    let tcp_server = serve(state.clone(), format!("{}:{}", addr, tcp_port));
    let http_server = http_serve(state.clone(), format!("{}:{}", addr, http_port));

    let runtime = tokio::runtime::Runtime::new().unwrap();

    runtime.spawn(tcp_server);
    runtime.spawn(http_server);

    if let Some(secs) = timeout {
        runtime.shutdown_timeout(Duration::from_secs(secs));
    }

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

        async move {
           Ok::<_, Infallible>(service_fn(move |req| {
               hello_world(state.clone(), req)
           }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);
    match server.await {
        Ok(()) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

async fn hello_world(_state: Arc<Mutex<State>>, _req: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(format!("much v{}", VERSION).into()))
}
