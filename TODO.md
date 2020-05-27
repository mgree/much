- [ ] Persistence

- [ ] Registration
  take email or twitter handle
  name, pronouns

- [ ] Options
  + [ ] passwords!
  + [ ] arrive/depart announcements
  + [ ] admin bit
  + [ ] permissions

- [ ] Logging
      https://docs.rs/tokio-trace/0.1.0/tokio_trace/

- [x] Command framework
  + [ ] Parsers
  + [ ] Documentation/help
  + Specific commands:
    * [ ] directed speech
    * [ ] emotes (manual and prefab)
    * [ ] private speech (tell)
    * [ ] mute
    * [ ] kick
    * [ ] ban 
          id, ip, ip range... timer?
    * [ ] global chat
    * [ ] announce
    * [ ] who
    * [ ] help
    * [ ] look
    * [ ] profile info/editing
    * [x] shutdown

- [ ] Maps/rooms
  + [ ] Library for parsing some file format

- [ ] HTTP gateway
      https://dev.to/deciduously/skip-the-framework-build-a-simple-rust-api-with-hyper-4jf5
      https://github.com/hyperium/hyper/blob/master/examples/multi_server.rs
      https://github.com/hyperium/hyper/blob/master/examples/single_threaded.rs

- [ ] Security issues
  + [ ] DoS/flood control
        https://docs.rs/tokio/0.2.21/tokio/time/fn.throttle.html
        doesn't seem to work directly on TCPStreams
  + [ ] CSRF protection
  + [ ] email verification
  + [ ] twitter verification

- [ ] Testing
  + [ ] Functionality
        Mocking?
  + [ ] Under load
