extern crate much;

use std::convert::Infallible;
use std::error::Error;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use clap::{App, Arg};

use tokio::sync::Mutex;

use tracing::{info, Level};
use tracing_subscriber;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};

use much::*;

const NAME   : &'static str = env!("CARGO_PKG_NAME");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

fn main() -> Result<(), Box<dyn Error>> {
    let config = App::new(NAME)
        .version(VERSION)
        .author(AUTHORS)
        .about("Multi-user conference hall")
        .arg(
            Arg::with_name("timeout")
                .short("t")
                .long("timeout")
                .takes_value(true)
                .value_name("SECONDS")
                .default_value("forever")
                .help("Time after which the server will shutdown"),
        )
        .arg(
            Arg::with_name("addr")
                .short("b")
                .long("bind")
                .takes_value(true)
                .value_name("ADDRESS")
                .default_value("0.0.0.0")
                .help("Sets the interface to listen on"),
        )
        .arg(
            Arg::with_name("TCP port")
                .long("tcp-port")
                .takes_value(true)
                .value_name("PORT")
                .default_value("4000")
                .help("Sets the port to listen for direct TCP connections on"),
        )
        .arg(
            Arg::with_name("HTTP port")
                .long("http-port")
                .takes_value(true)
                .value_name("PORT")
                .default_value("4080")
                .help("Sets the port to listen for HTTP connections on"),
        )
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .get_matches();

    let addr = config.value_of("addr").unwrap();
    let tcp_port = config.value_of("TCP port").unwrap();
    let http_port = config.value_of("HTTP port").unwrap();
    let timeout: Option<u64> = config.value_of("timeout").unwrap().parse().ok();

    let verbosity = match config.occurrences_of("v") {
        0 => Level::INFO,
        1 => Level::DEBUG,
        2 | _ => Level::TRACE,
    };

    // initialize logging
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // TODO log to a file?
        .with_max_level(verbosity)
        .init();

    info!("much v{}", VERSION);

    let state = Arc::new(Mutex::new(State::new()));
    info!("loaded state");

    let tcp_addr = format!("{}:{}", addr, tcp_port);
    let tcp_server = tcp_serve(state.clone(), tcp_addr.clone());
    let http_addr = format!("{}:{}", addr, http_port);
    let http_server = http_serve(state.clone(), http_addr.clone());

    let runtime = tokio::runtime::Runtime::new()?;
    info!("initialized tokio runtime");

    runtime.spawn(tcp_server);
    info!("started TCP server on {}", tcp_addr);

    runtime.spawn(http_server);
    info!("started HTTP server on {}", http_addr);

    if let Some(secs) = timeout {
        info!("shutdown timer: {} seconds", secs);
        runtime.shutdown_timeout(Duration::from_secs(secs));
    } else {
        loop {}
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
