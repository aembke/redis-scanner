// public interface: parse argv, inspect, ttls,
//
// utils: scan_node()

use crate::argv::Argv;
use fred::prelude::*;

/// A cluster node identifier and any information necessary to connect to it.
pub struct ClusterNode {
  pub server:  Server,
  pub builder: Builder,
}

/// Initialize the client and discover any other servers to be scanned.
pub async fn init(argv: &Argv) -> Result<(RedisClient, Vec<ClusterNode>), RedisError> {
  status!("Discovering servers...");

  let (builder, client) = utils::init(argv).await?;
  let nodes = utils::discover_servers(argv, &client)
    .await?
    .into_iter()
    .map(|server| {
      let builder = utils::change_builder_server(&builder, &server)?;
      Ok(ClusterNode { server, builder })
    })
    .collect::<Result<Vec<_>, RedisError>>()?;

  Ok((client, nodes))
}

///
pub mod argv;
///
pub mod idle;
///
pub mod memory;
pub mod progress;
///
pub mod touch;
///
pub mod ttl;
pub mod utils;
