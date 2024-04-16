# https://github.com/docker/for-mac/issues/5548#issuecomment-1029204019
# FROM rust:1.77-slim-buster
FROM rust:1.77-slim-bullseye
WORKDIR /project

ARG RUST_LOG
ARG REDIS_VERSION
ARG REDIS_USERNAME
ARG REDIS_PASSWORD

RUN USER=root apt-get update && apt-get install -y build-essential libssl-dev dnsutils curl pkg-config cmake
RUN echo "REDIS_VERSION=$REDIS_VERSION"

# For debugging
RUN cargo --version && rustc --version