# MUCH: Multi-User Conference Hall

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

Log, for each user, everything they see or hear?
  Single global log, with markers for who's heard it?
  Starts to feel invasive.

  What about private rooms?

A log of all tells. Optional conversation logging.

## Plans

OCaml seems like it will provide the easiest way to work.

Use Lwt or Lwt_unix? https://github.com/mirage/ocaml-cohttp for HTTP?
https://www.baturin.org/code/lwt-counter-server/ is a start
MirageOS feels like overkill.

Flat-file organization, like ROM MUD pfiles?  Could even hardcode
rooms to start (but need to create private rooms on the fly).

Do we need the 'tick' infrastructure that is the default on MUDs? We
have to worry a bit about DoS, but maybe we can do something simpler,
like IRC flood control?
