#!/usr/bin/env bash
# Automatically rebuilds and re-ups when files change
# Requires nodemon
nodemon -e rs,txt,toml,html -x "spin build && RUST_LOG=warn spin up"