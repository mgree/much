- [ ] Give up on multi-presence
      HTTP connections should just be able to be in one place at a time
      loading a new location URL is equivalent to running `go URL`
      prompt on multi-connect to disconnect other connection
      simplify state!

- [ ] Performance questions
  Switch from Mutex to RwLock? Don't want to block out the writers...
  Use parking_lot's RwLock (but maybe let's not switch to nightly)

- [ ] Persistence

- [ ] Registration
  + [x] take email or twitter handle
  + [ ] real name
  + [ ] screen name?
  + [ ] pronouns

- [ ] Options
  + [x] passwords!
  + [ ] arrive/depart announcements
  + [ ] admin bit
  + [ ] permissions

- [x] Logging
  + [ ] to a file
  + [ ] both at once?

- [x] Command framework
  + [ ] Proper parser, with error messages, etc.
  + [ ] Documentation/help
  + Specific commands:
    * [ ] directed speech
    * [ ] emotes (manual and prefab)
    * [ ] private speech (tell)
    * [ ] mute (get no events from); [ ] block (send no events to?)
        make sure this gets charged to the _muter_/_blocker_'s thread (i.e., filter on message receipt)
    * [ ] kick
    * [ ] ban
          id, ip, ip range... timer?
    * [ ] global chat
    * [ ] announce
    * [ ] who
    * [ ] where?
          harrassment?!
    * [ ] help
    * [ ] look
    * [ ] invite (to a private room)
    * [ ] transfer (ownership of a private room)
    * [ ] profile info/editing
    * [ ] status (afk/in a meeting/invisible)
    * [x] shutdown

- [x] Maps/rooms
  + [ ] movement commands
    * [ ] relative movement
    * [ ] go takes a room name or URL
  + [ ] keep a global, read-only map in for fast routing
  + [ ] private room creation
  + [ ] Library for parsing some file format

- [x] HTTP gateway
  + [x] Managing state in hyper
  + [x] Routing
  + [ ] Sessions
    * [x] in-memory tracking
    * [ ] logs per session of IP accesses on file system (write-only?)
  + [ ] Figure out API, set up interactions
    /api/be drains the queue for that peer (which is room associated)

|Route                |Function                                          |
|:--------------------|:-------------------------------------------------|
|/                    |if logged in, redirect to lobby; otherwise login  |
|/register            |registration form (GET form, POST results         |
|/user/<PERSONID>     |user profile page                                 |
|/room?<ROOMID>       |interact in given room (or redirect to login)     |
|/room?<ROOMNAME>     |interact in given room (or redirect to login)     |
|/who                 |listing of who is online                          |
|/help                |help pages (generate command docs automatically?) |
|/admin               |admin console?                                    |
|/api/do?<ROOMID>     |POST commands                                     |
|/api/be?<ROOMID>     |GET to poll (at least every 30s to stay logged in)|
|/api/leave?<ROOMID>  |POST to exit room (window.onclose, etc.)          |
|/api/login           |POST login info                                   |
|/api/logout          |logout                                            |
|/api/who             |current listing of who is online                  |

We probably _shouldn't_ let people know where people are... but surely we want to know if they're logged in?
The `who` command will do that in any case. Do we even want a `where` command?

- [ ] Security issues
  + [ ] DoS/flood control
        https://docs.rs/tokio/0.2.21/tokio/time/fn.throttle.html
        doesn't seem to work directly on TCPStreams
  + [ ] multi-login via TCP: keep newest, kick oldest
        needs new PeerMessage
  + [ ] CSRF protection
        only for state-changing operations (do, login, logout, leave)
        double-submit cookies (session ID needs to be a hidden form value?)
  + [ ] email verification
  + [ ] twitter verification
  + For profile information, we should be very clear about what we give out:
    * Name
    * Twitter handle
    * Online status
    * Location (option to turn off?)

- [ ] Testing!!!
  + [ ] Functionality
        Mocking?
  + [ ] Under load
