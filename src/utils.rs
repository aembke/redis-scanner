use crate::{argv::Argv, progress::global_progress};
use chrono::Duration;
use fred::{
  prelude::*,
  types::{Builder, ClusterDiscoveryPolicy, RedisConfig, ReplicaConfig, Server, ServerConfig, UnresponsiveConfig},
};
use log::debug;
use std::{
  collections::HashMap,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration as StdDuration,
};

#[cfg(all(feature = "enable-native-tls", feature = "enable-rustls"))]
compile_error!("Cannot use both TLS dependencies.");

#[cfg(any(feature = "enable-native-tls", feature = "enable-rustls"))]
use fred::types::{TlsConfig, TlsConnector};

pub const DEFAULT_ESTIMATE_UPDATE_INTERVAL: StdDuration = StdDuration::from_millis(2500);

pub fn incr_atomic(val: &AtomicUsize, amt: usize) -> usize {
  val.fetch_add(amt, Ordering::Acquire).saturating_add(amt)
}

pub fn read_atomic(val: &AtomicUsize) -> usize {
  val.load(Ordering::SeqCst)
}

pub fn set_atomic(val: &AtomicUsize, amt: usize) -> usize {
  val.swap(amt, Ordering::AcqRel)
}

#[cfg(all(feature = "enable-native-tls"))]
fn build_tls_config(argv: &Argv) -> Result<Option<TlsConfig>, RedisError> {
  //
  Ok(None)
}

#[cfg(feature = "enable-rustls")]
fn build_tls_config(argv: &Argv) -> Result<Option<TlsConfig>, RedisError> {
  //
  Ok(None)
}

/// Discover any other servers that should be scanned.
pub async fn discover_servers(argv: &Argv, client: &RedisClient) -> Result<Vec<Server>, RedisError> {
  let config = client.client_config();

  Ok(if client.is_clustered() {
    // use cached primary nodes or one replica per primary
    let primary_nodes = client
      .cached_cluster_state()
      .map(|state| state.unique_primary_nodes())
      .ok_or_else(|| RedisError::new(RedisErrorKind::Config, "Missing cluster state."))?;

    if argv.replicas {
      // pick one replica per primary node
      let replicas = client.replicas().nodes();
      let mut inverted = HashMap::with_capacity(primary_nodes.len());
      for (replica, primary) in replicas.into_iter() {
        let _ = inverted.insert(primary, replica);
      }

      inverted.into_values().collect()
    } else {
      primary_nodes
    }
  } else if config.server.is_sentinel() {
    client.active_connections().await?
  } else {
    config.server.hosts()
  })
}

/// Create a new `Builder` instance by replacing the destination server with a centralized server config to the
/// provided `server`.
pub fn change_builder_server(builder: &Builder, server: &Server) -> Result<Builder, RedisError> {
  let config = builder
    .get_config()
    .map(|config| {
      let mut config = config.clone();
      config.server = ServerConfig::Centralized { server: server.clone() };
      config
    })
    .ok_or_else(|| RedisError::new(RedisErrorKind::Config, "Invalid client builder."))?;

  let mut out = Builder::from_config(config);
  out.set_connection_config(builder.get_connection_config().clone());
  out.set_performance_config(builder.get_performance_config().clone());
  Ok(out)
}

/// Create a client builder and the initial connection to the server.
pub async fn init(argv: &Argv) -> Result<(Builder, RedisClient), RedisError> {
  let server = if let Some(ref service) = argv.sentinel_service {
    ServerConfig::Sentinel {
      hosts:        vec![Server::new(&argv.host, argv.port)],
      service_name: service.clone(),
    }
  } else if argv.cluster || argv.replicas {
    ServerConfig::Clustered {
      hosts:  vec![Server::new(&argv.host, argv.port)],
      policy: ClusterDiscoveryPolicy::ConfigEndpoint,
    }
  } else {
    ServerConfig::Centralized {
      server: Server::new(&argv.host, argv.port),
    }
  };
  let config = RedisConfig {
    server,
    #[cfg(any(feature = "enable-native-tls", feature = "enable-rustls"))]
    tls: build_tls_config(argv)?,
    username: argv.username.clone(),
    password: argv.password.clone(),
    database: argv.db.clone(),
    fail_fast: true,
    ..Default::default()
  };

  let mut builder = Builder::from_config(config);
  builder
    .with_connection_config(|config| {
      config.unresponsive = UnresponsiveConfig {
        max_timeout: Some(StdDuration::from_millis(10_000)),
        interval:    StdDuration::from_millis(2_000),
      };

      if argv.replicas {
        config.replica = ReplicaConfig {
          lazy_connections: true,
          primary_fallback: true,
          ignore_reconnection_errors: true,
          connection_error_count: 1,
          ..Default::default()
        };
      }
    })
    .set_policy(ReconnectPolicy::new_exponential(0, 100, 10_000, 2));

  let client = builder.build()?;
  client.init().await?;
  Ok((builder, client))
}

pub fn pretty_duration(dur: &Duration) -> String {
  let num_seconds = dur.num_seconds();
  let seconds = num_seconds % 60;
  let minutes = (num_seconds / 60) % 60;
  let hours = (num_seconds / 60 / 60) % 24;
  let days = num_seconds / 60 / 60 / 24;

  if days > 0 {
    format!("{days}d {hours}h {minutes}m {seconds}s")
  } else if hours > 0 {
    format!("{hours}h {minutes}m {seconds}s")
  } else if minutes > 0 {
    format!("{minutes}m {seconds}s")
  } else {
    format!("{seconds}s")
  }
}

pub async fn update_estimate(server: Server, client: RedisClient) -> Result<(), RedisError> {
  loop {
    tokio::time::sleep(DEFAULT_ESTIMATE_UPDATE_INTERVAL).await;

    debug!("Updating estimate for {}", server);
    let estimate: u64 = client.dbsize().await?;
    global_progress().update_estimate(&server, estimate);
  }
}
