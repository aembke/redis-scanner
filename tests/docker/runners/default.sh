#!/bin/bash

cargo test --release --lib --tests --features "network-logs" -- --test-threads=1 "$@"