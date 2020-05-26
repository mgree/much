# MUCH: Multi-User Conference Hall

[![Build Status](https://travis-ci.com/mgree/much.svg?branch=master)](https://travis-ci.com/mgree/much)

A text-based virtual conference venue, accessible via telnet, MUD
clients, or over the web.

## Main idea

A multi-user, located environment where the rooms correspond to the
relevant rooms of a conference hall: a registration desk, a lobby, a
hallways or two, lecture halls, and some private rooms/secluded
spaces. Maybe a hotel bar?

Think: IRC where you can only be in one place at a time.

Think: a MUD without 'mechanics' that's linked up to the conference
schedule and events.

In an ideal world, this tool would replace sli.do (for question
asking) and Slack (for general communication).

The 'room' mechanism will allow for serendipitous hallway-track
interaction. Being text-based means there's a low technological bar
for entry (unlike VR), and also allows for "lurking"---just like how
people sit and work on their laptos.

## Desiderata

### Web interface

There should be a 1-1 mapping of rooms to URLs. You can enter a URL
into the MUD client and you'll go right there.

How should session management work?

What happens when you open multiple room URLs?

Room descriptions should make it easy to share a link to "go here
immediately".

Private rooms are password/identity protected.
  How does management for that work?
  How long do they stick around?

### Identity

Anonymous, easy use has to be an option. Web users are anonymous by
default.

Let people enter an email address and confirm to get privileges (e.g.,
create private rooms) and also have badges associated with their names
(author, presenter, organizer, etc.)

### Admin controls

Ban individuals, IPs, IP ranges.

### Announcements

Announce globally whatever @POPLconf tweets.

Some way of setting a schedule, and having announcements happen there?

### Logs/history

Maybe no logs for users? Just server-side event logs, let people sort
it out themselves. MUD clients log well, and we can offer downloads in
the HTML view.

We _do_ need to keep careful track of resource use.

## Plans

- Tokio
https://github.com/tokio-rs/tokio/blob/master/examples/chat.rs
- Hyper
  CSRF protection?

### Protocol fanciness

#### MCCP

https://docs.rs/flate2/1.0.14/flate2/
https://tintin.mudhalla.net/protocols/exop/
MCCP2 is code 86

#### Color

https://en.wikipedia.org/wiki/ANSI_escape_code
https://mudhalla.net/tintin/info/ansicolor/

### Database

Flat-file organization, like ROM MUD pfiles?  Could even hardcode
rooms to start (but need to create private rooms on the fly).

### Game state

World map
  Rooms know who's in them
  
Session IDs map to:
  player (how are anonymous people represented... HTTP session UUID?)
  
  HTTP pull drains buffer
