use crate::{
  argv::{Argv, Commands, ExpireArgv},
  clear_status,
  output::Output,
  progress::{self, global_progress, setup_event_logs, Counters},
  status,
  utils,
  ClusterNode,
  Command,
};
use fred::{
  prelude::*,
  types::{ClusterHash, CustomCommand},
};
use log::{debug, trace, warn};
use regex::Regex;
use std::{future::Future, sync::Arc, time::Duration};

static DRY_RUN_PREFIX: &str = "[Dry Run]";

#[derive(Clone)]
pub struct State {
  pub argv:     Arc<Argv>,
  pub cmd_argv: Arc<ExpireArgv>,
  pub counters: Arc<Counters>,
}

async fn scan_node(state: &State, server: Server, client: RedisClient) -> Result<(usize, usize), RedisError> {
  let expire_cmd = CustomCommand::new_static("EXPIRE", ClusterHash::FirstKey, false);
  let scanner = client.scan(&state.argv.pattern, Some(state.argv.page_size), None);
  let filter = state.argv.filter.as_ref().and_then(|s| Regex::new(s).ok());
  let reject = state.argv.reject.as_ref().and_then(|s| Regex::new(s).ok());

  utils::scan_server(
    server.clone(),
    state.argv.ignore,
    state.argv.delay,
    scanner,
    move |mut scanned, mut success, mut skipped, mut errored, keys| {
      let (filter, reject, client, server, expire_cmd) = (
        filter.clone(),
        reject.clone(),
        client.clone(),
        server.clone(),
        expire_cmd.clone(),
      );

      async move {
        state.counters.incr_scanned(keys.len());
        scanned += keys.len();

        let keys: Vec<_> = keys
          .into_iter()
          .filter(|key| {
            if utils::should_skip_key_by_regexp(&filter, &reject, key) {
              trace!("{} Filtering out key {}", server, key.as_str_lossy());
              skipped += 1;
              state.counters.incr_skipped(1);
              false
            } else {
              true
            }
          })
          .collect();

        if state.cmd_argv.really {
          if !keys.is_empty() {
            debug!("{} Calling EXPIRE on {} keys...", server, keys.len());
            let pipeline = client.pipeline();
            for key in keys.iter() {
              if let Some(ref arg) = state.cmd_argv.qualifier {
                pipeline
                  .custom(expire_cmd.clone(), vec![state.cmd_argv.seconds.into(), arg.to_arg()])
                  .await?;
              } else {
                pipeline.expire(key, state.cmd_argv.seconds).await?;
              }
            }
            let results = pipeline.try_all::<u32>().await;

            for result in results.into_iter() {
              if let Err(err) = result {
                warn!("{} Error with EXPIRE: {:?}", server, err);
                errored += 1;
                state.counters.incr_errored(1);
                if !state.argv.ignore {
                  return Err(err);
                }
              } else {
                success += 1;
                state.counters.incr_success(1);
              }
            }
          }
        } else {
          trace!("[Dry Run] Would have set expiration on {:?}", keys);
          state.counters.incr_success(keys.len());
          success += keys.len();
        }

        Ok((scanned, success, skipped, errored))
      }
    },
  )
  .await
}

pub struct ExpireCommand;

impl Command for ExpireCommand {
  fn run(
    argv: Arc<Argv>,
    _: RedisClient,
    nodes: Vec<ClusterNode>,
  ) -> impl Future<Output = Result<Option<Box<dyn Output>>, RedisError>> + Send {
    async move {
      let cmd_argv = match argv.command {
        Commands::Expire(ref inner) => Arc::new(inner.clone()),
        _ => return Err(RedisError::new(RedisErrorKind::Config, "Invalid command")),
      };

      if argv.replicas {
        return Err(RedisError::new(
          RedisErrorKind::Config,
          "Cannot expire keys on replica nodes.",
        ));
      }

      let mut tasks = Vec::with_capacity(nodes.len());
      let counters = Counters::new();
      let state = State {
        argv: argv.clone(),
        cmd_argv,
        counters,
      };
      let server_prefix = if state.cmd_argv.really {
        status!("`--really` provided. Sleeping for 5 seconds before starting...");
        tokio::time::sleep(Duration::from_secs(5)).await;
        status!("Connecting to servers...");
        None
      } else {
        status!(DRY_RUN_PREFIX, "Starting dry run...");
        Some(DRY_RUN_PREFIX)
      };

      progress::watch_totals(&state.counters);
      for node in nodes.into_iter() {
        let state = state.clone();
        tasks.push(tokio::spawn(async move {
          let client = node.builder.build()?;
          client.init().await?;

          let estimate: u64 = client.dbsize().await?;
          global_progress().add_server(&node.server, Some(estimate), server_prefix);
          let estimate_task = tokio::spawn(utils::update_estimate(node.server.clone(), client.clone()));
          let event_task = setup_event_logs(&client);

          let result = scan_node(&state, node.server, client).await;
          estimate_task.abort();
          event_task.abort();
          result.map(|_| ())
        }));
      }
      clear_status!();

      if let Err(err) = utils::wait_with_interrupts(tasks).await {
        eprintln!("Fatal error while scanning: {:?}", err);
      }
      Ok(None)
    }
  }
}
