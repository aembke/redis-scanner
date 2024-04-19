#!/bin/bash

docker-compose -f tests/docker/compose/clustered.yml \
  -f tests/docker/compose/centralized.yml \
  -f tests/docker/compose/sentinel.yml \
  -f tests/docker/runners/default.yml \
  run -u $(id -u ${USER}):$(id -g ${USER}) --rm default-tests