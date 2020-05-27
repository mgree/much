- [ ] Performance questions
  Switch from Mutex to RwLock? Don't want to block out the writers...

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

- [x] HTTP gateway
  + [ ] Managing state

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
