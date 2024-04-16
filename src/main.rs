use clap::Parser;
use redis_scanner::{
  self,
  argv::{Argv, Commands},
  status,
};
use std::sync::Arc;
use tokio::runtime::Builder;

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() {
  pretty_env_logger::init();
  let argv = Arc::new(Argv::parse());
  debug!("Argv: {:?}", argv);
  let (client, nodes) = redis_scanner::init(&argv).await.expect("Failed to initialize");

  // TODO catch ctrl-c and print output so far
  match argv.command {
    Commands::Idle(ref inner) => redis_scanner::idle::run(&argv, inner, client, nodes).await.unwrap(),
    Commands::Ttl(ref inner) => redis_scanner::ttl::run(&argv, inner, client, nodes).await.unwrap(),
    Commands::Touch(ref inner) => redis_scanner::touch::run(&argv, inner, client, nodes).await.unwrap(),
    Commands::Memory(ref inner) => redis_scanner::memory::run(&argv, inner, client, nodes).await.unwrap(),
  }
}
