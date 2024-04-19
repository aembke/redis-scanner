#!/bin/bash

cargo test --release --lib --tests --features "network-logs enable-native-tls" -- --test-threads=1 "$@"