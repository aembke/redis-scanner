#!/bin/bash

cargo test --release --lib --tests --features "network-logs enable-rustls" -- --test-threads=1 "$@"