use fred::prelude::*;
use regex::Regex;
use std::{
  collections::{BTreeMap, BTreeSet, HashMap, HashSet},
  env,
  fs,
};

pub fn read_env_var(name: &str) -> Option<String> {
  env::var_os(name).and_then(|s| s.into_string().ok())
}

#[cfg(any(feature = "enable-rustls", feature = "enable-native-tls"))]
struct TlsCreds {
  root_cert_der:   Vec<u8>,
  root_cert_pem:   Vec<u8>,
  client_cert_der: Vec<u8>,
  client_cert_pem: Vec<u8>,
  client_key_der:  Vec<u8>,
  client_key_pem:  Vec<u8>,
}

#[cfg(any(feature = "enable-rustls", feature = "enable-native-tls"))]
fn check_file_contents(value: &Vec<u8>, msg: &str) {
  if value.is_empty() {
    panic!("Invalid empty TLS file: {}", msg);
  }
}

/// Read the (root cert.pem, root cert.der, client cert.pem, client cert.der, client key.pem, client key.der) tuple
/// from the test/tmp/creds directory.
#[cfg(any(feature = "enable-native-tls", feature = "enable-rustls"))]
fn read_tls_creds() -> TlsCreds {
  let creds_path = read_env_var("TEST_TLS_CREDS").expect("Failed to read TLS path from env");
  let root_cert_pem_path = format!("{}/ca.pem", creds_path);
  let root_cert_der_path = format!("{}/ca.crt", creds_path);
  let client_cert_pem_path = format!("{}/client.pem", creds_path);
  let client_cert_der_path = format!("{}/client.crt", creds_path);
  let client_key_der_path = format!("{}/client_key.der", creds_path);
  let client_key_pem_path = format!("{}/client.key8", creds_path);

  let root_cert_pem = fs::read(&root_cert_pem_path).expect("Failed to read root cert pem");
  let root_cert_der = fs::read(&root_cert_der_path).expect("Failed to read root cert der");
  let client_cert_pem = fs::read(&client_cert_pem_path).expect("Failed to read client cert pem");
  let client_cert_der = fs::read(&client_cert_der_path).expect("Failed to read client cert der");
  let client_key_der = fs::read(&client_key_der_path).expect("Failed to read client key der");
  let client_key_pem = fs::read(&client_key_pem_path).expect("Failed to read client key pem");

  check_file_contents(&root_cert_pem, "root cert pem");
  check_file_contents(&root_cert_der, "root cert der");
  check_file_contents(&client_cert_pem, "client cert pem");
  check_file_contents(&client_cert_der, "client cert der");
  check_file_contents(&client_key_pem, "client key pem");
  check_file_contents(&client_key_der, "client key der");

  TlsCreds {
    root_cert_pem,
    root_cert_der,
    client_cert_der,
    client_cert_pem,
    client_key_pem,
    client_key_der,
  }
}

#[cfg(feature = "enable-rustls")]
pub fn create_rustls_config() -> fred::types::TlsConnector {
  use fred::rustls::pki_types::PrivatePkcs8KeyDer;

  let creds = read_tls_creds();
  let mut root_store = fred::rustls::RootCertStore::empty();
  let _ = root_store
    .add(creds.root_cert_der.clone().into())
    .expect("Failed adding to rustls root cert store");

  let cert_chain = vec![creds.client_cert_der.into(), creds.root_cert_der.into()];

  fred::rustls::ClientConfig::builder()
    .with_root_certificates(root_store)
    .with_client_auth_cert(cert_chain, PrivatePkcs8KeyDer::from(creds.client_key_der).into())
    .expect("Failed to build rustls client config")
    .into()
}

#[cfg(feature = "enable-native-tls")]
pub fn create_native_tls_config() -> fred::types::TlsConnector {
  let creds = read_tls_creds();

  let root_cert = fred::native_tls::Certificate::from_pem(&creds.root_cert_pem).expect("Failed to parse root cert");
  let mut builder = fred::native_tls::TlsConnector::builder();
  builder.add_root_certificate(root_cert);

  let mut client_cert_chain = Vec::with_capacity(creds.client_cert_pem.len() + creds.root_cert_pem.len());
  client_cert_chain.extend(&creds.client_cert_pem);
  client_cert_chain.extend(&creds.root_cert_pem);

  let identity = fred::native_tls::Identity::from_pkcs8(&client_cert_chain, &creds.client_key_pem)
    .expect("Failed to create client identity");
  builder.identity(identity);

  builder.try_into().expect("Failed to build native-tls connector")
}

pub async fn load_ipsum_file_word_counts(client: &RedisClient) -> Result<BTreeMap<&'static str, u64>, RedisError> {
  let contents = include_str!("../docker/ipsum.txt");
  let mut words: BTreeMap<&str, u64> = BTreeMap::new();
  for word in contents.split(" ") {
    if word.trim().is_empty() {
      continue;
    }

    let entry = words.entry(word).or_insert(0);
    *entry += 1;
  }

  for (word, count) in words.iter() {
    client.set(*word, *count, None, None, false).await?;
  }
  Ok(words)
}

pub async fn load_moby_dick_word_counts(client: &RedisClient) -> Result<BTreeMap<String, u64>, RedisError> {
  let contents = include_str!("../docker/moby-dick.txt").replace("\n", " ");
  let mut words: BTreeMap<String, u64> = BTreeMap::new();
  for word in contents.split(" ") {
    let word = word.replace("\t", "");
    if word.trim().is_empty() {
      continue;
    }

    let entry = words.entry(word).or_insert(0);
    *entry += 1;
  }

  for (word, count) in words.iter() {
    client.set(word, *count, None, None, false).await?;
  }
  Ok(words)
}

// pub async fn load_counter(
// client: &RedisClient,
// prefix: &str,
// suffix: &str,
// count: usize,
// ) -> Result<HashMap<RedisKey, usize>, RedisError> {
// TODO use this to test group by
// let mut out = HashMap::with_capacity(count);
// for idx in 0 .. count {
// let key: RedisKey = format!("{}{}{}", prefix, count, suffix).into();
// client.set(key.clone(), idx, None, None, false).await?;
// out.insert(key, idx);
// }
//
// Ok(out)
// }
