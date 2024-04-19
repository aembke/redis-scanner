#!/bin/bash

docker-compose -f tests/docker/compose/clustered-tls.yml -f tests/docker/runners/default-rustls.yml \
  run -u $(id -u ${USER}):$(id -g ${USER}) --rm default-rustls-tests