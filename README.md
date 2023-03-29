# Spin RTC Sigaling Demo

A
[WebRTC signaling](https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Connectivity#signaling) server
in [Rust](https://www.rust-lang.org/)
for [Fermyon Spin](https://github.com/fermyon/spin).

## Getting Started

1. Sign up on [Fermyon Cloud](https://cloud.fermyon.com/) and [set up Spin](https://developer.fermyon.com/spin/quickstart/)
2. Set up a [Redis](https://redis.io/) server and place the URL into a file named `redis.env` in the project root
3. Run `./deploy.sh` to deploy the demo app
    - Or use `./up.sh` to run locally
4. Visit your Fermyon URL in two different tabs
    - You can do this on a single computer, or between different computers
5. One tab will "host" the session, with the "Request new session" button
6. The session name can be sent to the other tab to use for the "Join Session" section
7. A connection should be established and you will see the "Hello from..." messages on both sides