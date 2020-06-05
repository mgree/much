extern crate much;

use futures_util::sink::SinkExt;
use much::*;
use tokio::stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

fn config_timeout(timeout: u64) -> Config {
    let mut config = Config::default();
    config.timeout = Some(timeout);
    config
}

async fn simple_state() -> GameState {
    let state = much::init();

    {
        let mut state = state.lock().await;

        let _ = state.new_person("@a", "aaaaaaaa");
        let _ = state.new_person("@b", "bbbbbbbb");
        let _ = state.new_person("@c", "cccccccc");
    }
    state
}

#[tokio::test]
async fn successful_login_and_shutdown() {
    let config = config_timeout(1);
    let state = simple_state().await;

    let tcp_server = tcp_serve(state.clone(), config.tcp_addr());

    tokio::spawn(tcp_server);
    tokio::time::delay_for(tokio::time::Duration::from_millis(30)).await;

    let stream = tokio::net::TcpStream::connect(config.tcp_addr())
        .await
        .expect("connected");
    let mut lines = Framed::new(stream, LinesCodec::new());

    let _prompt = lines.next().await.expect("username prompt");
    lines.send("@a").await.expect("send username");
    let _prompt = lines.next().await.expect("password prompt");
    lines.send("aaaaaaaa").await.expect("send login");
    let _prompt = lines.next().await.expect("logged in message");
    lines.send("shutdown").await.expect("send shutdown comand");

    tokio::time::delay_for(tokio::time::Duration::from_millis(30)).await;

    let done = lines.next().await;

    match done {
        Some(Ok(line)) => assert_eq!(line, ""),
        Some(Err(_e)) => return (),
        None => return (),
    }

    let done = lines.next().await;

    match done {
        Some(Ok(line)) => panic!("expected empty line, got '{}'", line),
        Some(Err(_e)) => return (),
        None => return (),
    }
}

