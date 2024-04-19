#!/bin/bash

docker-compose -f tests/docker/compose/clustered-tls.yml -f tests/docker/runners/default-native-tls.yml \
  run -u $(id -u ${USER}):$(id -g ${USER}) --rm default-native-tls-tests