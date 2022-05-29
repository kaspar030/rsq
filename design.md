# Design thoughts

This will be updated randomly.

## Use-cases

### secure notifications

e.g., there's a channel for docker hub pushes. On docker push,
  the CI action sends a message to a defined channel (e.g., using rsq cli).
  Elsewhere, subscribers can then act on this message.
  => Web hooks replacement.

### distributed applications

#### Git Friends

e.g., have "Git friends". On push, rsq cli sends a message to friends.
Friends automatically "fetch" the corresponding remote. Possibly, even this can
be transported (by implementing a "git transport") over rsq channels.

#### networked copy&paste

copied data on one computer could get send to a "copy\&paste queue" for a user.
copy on one computer, paste on the other!

#### file transfer

have a "friend". to `rsq sendfile <friend>`. friend receives file. all encrypted,
using pre-made friend connections.

#### automatically updating contacts

"friends" share their contact endpoint. someone changes their phone number, location,
work email address -\> notifies their friends -\> address books update automatically

### workflows

#### Sending a message

1. PeerConnection receives msg via socket
2. Hub looks at message, gets Channel
3. Hub calls Channel.send(msg)
4. Channel iterates subscribed PeerConnections, calls conn.send(msg)

## Threat analysis

## Privacy

- home servers are trusted with metadata
- only endpoints can see content
