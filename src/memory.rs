use crate::{
  argv::{Argv, MemoryArgv},
  ClusterNode,
};
use fred::prelude::*;
use std::sync::Arc;

pub async fn run(
  argv: &Arc<Argv>,
  cmd_argv: &MemoryArgv,
  client: RedisClient,
  nodes: Vec<ClusterNode>,
) -> Result<(), RedisError> {
  unimplemented!()
}
