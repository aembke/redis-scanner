#!/bin/bash

declare -a targets=(
  "aarch64-unknown-linux-gnu"
  "aarch64-unknown-linux-musl"
  "x86_64-unknown-linux-gnu"
  "x86_64-unknown-linux-musl"
)

for target in "${targets[@]}"
do
  Echo "Building for $target"
  cross build --release --bins --features "enable-native-tls vendored-openssl" --target $target
done

rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
cargo build --release --bin redis_scanner --features "enable-native-tls vendored-openssl" --target x86_64-apple-darwin
cargo build --release --bin redis_scanner --features "enable-native-tls vendored-openssl" --target aarch64-apple-darwin

mkdir tests/tmp/releases
for target in "${targets[@]}"
do
  cp "target/$target/release/redis_scanner" "tests/tmp/releases/redis_scanner-$target"
done

cp target/x86_64-apple-darwin/release/redis_scanner tests/tmp/releases/redis_scanner-x86_64-apple-darwin
cp target/aarch64-apple-darwin/release/redis_scanner tests/tmp/releases/redis_scanner-aarch64-apple-darwin

echo "Finished building."