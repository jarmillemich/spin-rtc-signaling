## The flow

1. Alice says they're going to host a session
    - Specifies public/private
    - Gets the session name
    - Server generates a secret for Alice to authenticate further actions
2. Bob gets the session name via either
    - the public list 
    - an out-of-band channel
3. Bob says they're going to join that session
    - Creates an RTC offer
    - Sends their name
        - If name is already taken, reject (keyed by session name)
    - Server generates a secret for Bob to authenticate further actions
    - Server queues up the join request details for Alice to pick up
4. Alice has been polling the join queue and gets the join offer
    - Creates a local RTC connection with the offer
    - Posts back to the server the RTC answer
5. After each party sends their offer/answer, they also send ICE candidates
    - Send in batches, every n seconds or so
    - Each party also polls for ICE candidates until the connection is made
    - Once the connection is made, can we stop submitting ICE candidates?
6. Bob is now joined to the session and there is a P2P connection Alice<->Bob
    - All future communications happen over this channel
    - Notify the server and remove related info?
        - Or just expire after a couple minutes

## State

- Session state
    - Session name
    - Host name
    - Host secret
    - Clients?
- Connection startup state
    - Do we have two buckets, one to host and one to client?
    - Session name
    - Client name
    - Client secret
    - Client offer
    - Host answer
    - ICE candidates (either direction)

### Storage

- Have all session properties in a hash
    - keyed by session name
    - Includes host name/secret
- Have a set of client names in a set
    - keyed by session name
- Have connection messages to the host (offer, ICE) in a list
    - keyed by session name
- Have connection messages to a client (answer, ICE) in a list
    - keyed by session name, client name
- Have connection request secrets in values
    - keyed by session name, client name

