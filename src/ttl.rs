use crate::{
  argv::{Argv, TtlArgv},
  ClusterNode,
};
use fred::prelude::*;
use std::sync::Arc;

pub async fn run(
  argv: &Arc<Argv>,
  cmd_argv: &TtlArgv,
  client: RedisClient,
  nodes: Vec<ClusterNode>,
) -> Result<(), RedisError> {
  unimplemented!()
}
