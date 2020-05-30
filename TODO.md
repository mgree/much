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
  + [ ] passwords!
  + [ ] arrive/depart announcements
  + [ ] admin bit
  + [ ] permissions

- [x] Logging
  + [ ] to a file

- [x] Command framework
  + [ ] Proper parser, with error messages, etc.
  + [ ] Documentation/help
  + Specific commands:
    * [ ] directed speech
    * [ ] emotes (manual and prefab)
    * [ ] private speech (tell)
    * [ ] mute (get no events from)
    * [ ] block (send no events to?)
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
    * [x] shutdown

- [ ] Maps/rooms
  + [ ] movement commands
    * [ ] relative movement
    * [ ] go takes a room name or URL
  + [ ] keep a global, read-only map in for fast routing
  + [ ] private room creation
  + [ ] Library for parsing some file format

- [x] HTTP gateway
  + [x] Managing state
  + [ ] Figure out API, set up interactions

|Route                |Function                                          |
|:--------------------|:-------------------------------------------------|
|/                    |if logged in, redirect to lobby; otherwise login  |
|/room/<ROOM>         |interact in given room (or redirect to login)     |
|/room/<ROOM>/do      |POST commands                                     |
|/room/<ROOM>/be      |GET to poll (at least very 30s to stay logged in) |
|/register            |registration form                                 |
|/login               |POST login info                                   |
|/logout              |logout                                            |
|/user/<PERSONID>     |user profile page                                 |
|/help                |help pages (generate command docs automatically?) |
|/admin               |admin console?                                    |

Use https://docs.rs/tokio/0.2.21/tokio/time/struct.DelayQueue.html for /be

We probably _shouldn't_ let people know where people are... but surely we want to know if they're logged in?
The `who` command will do that in any case. Do we even want a `where` command?

- [ ] Security issues
  + [ ] DoS/flood control
        https://docs.rs/tokio/0.2.21/tokio/time/fn.throttle.html
        doesn't seem to work directly on TCPStreams
  + [ ] multi-login via TCP: keep newest, kick oldest
        needs new PeerMessage
  + [ ] CSRF protection
  + [ ] email verification
  + [ ] twitter verification
  + For profile information, we should be very clear about what we give out:
    * Name
    * Twitter handle
    * Online status
    * Location (option to turn off?)

- [ ] Testing
  + [ ] Functionality
        Mocking?
  + [ ] Under load
