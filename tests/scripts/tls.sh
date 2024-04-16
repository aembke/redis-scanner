#!/bin/bash

function check_root_dir {
  if [ ! -d "$ROOT/tests/tmp" ]; then
    echo "Must be in application root for redis installation scripts to work."
    exit 1
  fi
}

function check_cluster_credentials {
  if [ -f "$ROOT/tests/tmp/creds/ca.pem" ]; then
    echo "Skip generating TLS credentials."
    return 1
  else
    echo "TLS credentials not found."
    return 0
  fi
}

# Generate creds for a CA, a cert/key for the client, a cert/key for each node in the cluster, and sign the certs with the CA creds.
#
# Note: it's also necessary to modify DNS mappings so the CN in each cert can be used as a hostname. See `modify_etc_hosts`.
function generate_cluster_credentials {
  echo "Generating keys..."
  if [ ! -d "$ROOT/tests/tmp/creds" ]; then
    mkdir -p $ROOT/tests/tmp/creds
  fi
  pushd $ROOT > /dev/null
  cd $ROOT/tests/tmp/creds
  rm -rf ./*

  echo "Generating CA key pair..."
  openssl req -new -newkey rsa:2048 -nodes -out ca.csr -keyout ca.key -subj '/CN=redis-cluster'
  openssl x509 -signkey ca.key -days 90 -req -in ca.csr -out ca.pem
  # need the CA cert in DER format for rustls
  openssl x509 -outform der -in ca.pem -out ca.crt

  echo "Generating client key pair..."
  # native-tls wants a PKCS#8 key and redis-cli wants a PKCS#1 key
  openssl genrsa -out client.key 2048
  openssl pkey -in client.key -out client.key8
  # rustls needs it in DER format
  openssl rsa -in client.key -inform pem -out client_key.der -outform der

  openssl req -new -key client.key -out client.csr -subj '/CN=client.redis-cluster'
  openssl x509 -req -days 90 -sha256 -in client.csr -CA ca.pem -CAkey ca.key -set_serial 01 -out client.pem
  # need the client cert in DER format for rustls
  openssl x509 -outform der -in client.pem -out client.crt

  echo "Generating key pairs for each cluster node..."
  for i in `seq 1 6`; do
    # redis-server wants a PKCS#1 key
    openssl genrsa -out "node-$i.key" 2048
    # create SAN entries for all the other nodes
    openssl req -new -key "node-$i.key" -out "node-$i.csr" -config "$ROOT/tests/scripts/tls/node-$i.cnf"
    # might not work on os x with native-tls (https://github.com/sfackler/rust-native-tls/issues/143)
    openssl x509 -req -days 90 -sha256 -in "node-$i.csr" -CA ca.pem -CAkey ca.key -set_serial 01 -out "node-$i.pem" \
      -extensions req_ext -extfile "$ROOT/tests/scripts/tls/node-$i.cnf"
  done

  chmod +r ./*
  TLS_CREDS_PATH=$PWD
  popd > /dev/null
}
