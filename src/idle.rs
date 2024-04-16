use crate::{
  argv::{Argv, IdleArgv},
  ClusterNode,
};
use fred::prelude::*;
use std::sync::Arc;

pub async fn run(
  argv: &Arc<Argv>,
  cmd_argv: &IdleArgv,
  client: RedisClient,
  nodes: Vec<ClusterNode>,
) -> Result<(), RedisError> {
  unimplemented!()
}
