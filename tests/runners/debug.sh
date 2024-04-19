#!/bin/bash

# boot all the redis servers and start a bash shell on a new container
docker-compose -f tests/docker/compose/clustered-tls.yml \
  -f tests/docker/compose/centralized.yml \
  -f tests/docker/compose/clustered.yml \
  -f tests/docker/compose/sentinel.yml \
  -f tests/docker/compose/debug.yml run -u $(id -u ${USER}):$(id -g ${USER}) --rm debug