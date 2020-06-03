use std::error::Error;

use tracing;
use tracing_subscriber;

use much;

fn main() -> Result<(), Box<dyn Error>> {
    let config = much::Config::from_args();

    // initialize logging
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // TODO log to a file?
        .with_max_level(config.verbosity.clone())
        .init();

    tracing::info!("much v{}", much::VERSION);

    let state = much::init();
    tracing::info!("initialized fresh state");

    much::run(&config, state)
}
