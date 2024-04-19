mod utils;

use crate::integration::utils::load_ipsum_file_word_counts;
use fred::prelude::*;
use redis_scanner::{
  self,
  argv::{Argv, Commands, TouchArgv},
  ClusterNode,
  Command,
};
use std::{collections::BTreeMap, sync::Arc};

struct TlsTestArgv {
  pub key:     Option<String>,
  pub cert:    Option<String>,
  pub ca_cert: Option<String>,
}

struct TestArgv {
  pub host:             String,
  pub port:             u16,
  pub cluster:          bool,
  pub replicas:         bool,
  pub sentinel_service: Option<String>,
  pub tls:              Option<TlsTestArgv>,
  pub pattern:          String,
  pub filter:           Option<String>,
}

impl TestArgv {
  pub fn to_argv(&self, command: Commands) -> Argv {
    let (tls, tls_key, tls_cert, tls_ca_cert) = if let Some(tls) = self.tls.as_ref() {
      (true, tls.key.clone(), tls.cert.clone(), tls.ca_cert.clone())
    } else {
      (false, None, None, None)
    };

    Argv {
      host: self.host.clone(),
      port: self.port,
      cluster: self.cluster,
      replicas: self.replicas,
      sentinel_service: self.sentinel_service.clone(),
      pattern: self.pattern.clone(),
      filter: self.filter.clone(),
      quiet: true,
      tls,
      tls_key,
      tls_cert,
      tls_ca_cert,
      command,
      ..Default::default()
    }
    .fix()
  }
}

async fn init_with_ipsum(
  argv: &TestArgv,
  command: Commands,
) -> Result<(Arc<Argv>, RedisClient, Vec<ClusterNode>, BTreeMap<&'static str, u64>), RedisError> {
  let argv = Arc::new(argv.to_argv(command));
  let (client, nodes) = redis_scanner::init(&argv).await.unwrap();

  client.flushall_cluster().await?;
  let expected = load_ipsum_file_word_counts(&client).await.unwrap();
  Ok((argv, client, nodes, expected))
}

async fn should_touch_ipsum(argv: &TestArgv) {
  let (argv, client, nodes, _) = init_with_ipsum(argv, Commands::Touch(TouchArgv {})).await.unwrap();
  redis_scanner::touch::TouchCommand::run(argv, client, nodes)
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_ipsum_centralized() {
  let _ = pretty_env_logger::try_init();
  let argv = TestArgv {
    host:             "redis-main".into(),
    port:             6379,
    cluster:          false,
    replicas:         false,
    sentinel_service: None,
    tls:              None,
    pattern:          "*".into(),
    filter:           None,
  };

  should_touch_ipsum(&argv).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_ipsum_clustered() {
  let _ = pretty_env_logger::try_init();
  let argv = TestArgv {
    host:             "redis-cluster-1".into(),
    port:             30001,
    cluster:          true,
    replicas:         false,
    sentinel_service: None,
    tls:              None,
    pattern:          "*".into(),
    filter:           None,
  };

  should_touch_ipsum(&argv).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_ipsum_clustered_replicas() {
  let _ = pretty_env_logger::try_init();
  let argv = TestArgv {
    host:             "redis-cluster-1".into(),
    port:             30001,
    cluster:          true,
    replicas:         true,
    sentinel_service: None,
    tls:              None,
    pattern:          "*".into(),
    filter:           None,
  };

  should_touch_ipsum(&argv).await;
}
