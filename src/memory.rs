use crate::{
  argv::{Argv, Commands, MemoryArgv, OutputFormat},
  clear_status,
  output::Output,
  pqueue::{HashAndOrd, PrioQueue},
  progress,
  progress::{global_progress, setup_event_logs, Counters},
  status,
  utils,
  ClusterNode,
  Command,
};
use fred::prelude::*;
use log::{debug, error};
use regex::Regex;
use std::{
  borrow::Cow,
  future::Future,
  hash::{DefaultHasher, Hash, Hasher},
  sync::{atomic::AtomicUsize, Arc},
};

static HEADERS: &[&str] = &["Key", "Memory", "Percent Used"];
#[derive(Clone, Debug)]
pub struct Memory {
  pub key:          RedisKey,
  pub memory_usage: i64,
  pub group:        Option<String>,
}

impl Memory {
  pub fn group_or_key(&self) -> Cow<str> {
    self
      .group
      .as_ref()
      .map(|s| Cow::Borrowed(s.as_str()))
      .unwrap_or(self.key.as_str_lossy())
  }

  pub fn serialize(self, total: usize) -> Vec<String> {
    let used = if total == 0 {
      0.0
    } else {
      self.memory_usage as f64 / total as f64
    };

    vec![
      self.group_or_key().escape_default().to_string(),
      self.memory_usage.to_string(),
      format!("{:.2}", used * 100.0),
    ]
  }
}

impl HashAndOrd for Memory {
  fn weight(&self) -> i64 {
    self.memory_usage
  }

  fn int_hash(&self) -> u64 {
    // default hasher should be fine
    let mut h = DefaultHasher::new();
    if let Some(group) = self.group.as_ref() {
      'g'.hash(&mut h);
      group.as_bytes().hash(&mut h);
    } else {
      'k'.hash(&mut h);
      self.key.as_bytes().hash(&mut h);
    }
    h.finish()
  }

  fn merge(&mut self, other: Self) {
    self.memory_usage += other.memory_usage;
  }
}

#[derive(Clone)]
pub struct State {
  pub argv:       Arc<Argv>,
  pub cmd_argv:   Arc<MemoryArgv>,
  pub counters:   Arc<Counters>,
  pub pqueue:     Arc<PrioQueue<Memory>>,
  pub total_used: Arc<AtomicUsize>,
}

impl State {
  pub fn take(self: Box<Self>) -> (Vec<Memory>, usize) {
    let offset = self.cmd_argv.offset as usize;
    let results = Arc::try_unwrap(self.pqueue)
      .unwrap_or_else(|o| o.deep_copy())
      .into_vec();

    (results, offset)
  }
}

impl Output for State {
  fn format(&self) -> OutputFormat {
    self.cmd_argv.format.clone()
  }

  fn print_table(self: Box<Self>) -> String {
    let total = utils::read_atomic(&self.total_used);
    let (results, offset) = self.take();
    let rows: Vec<_> = results
      .into_iter()
      .skip(offset)
      .map(|memory| memory.serialize(total))
      .collect();

    utils::print_table(HEADERS, rows)
  }

  fn print_json(self: Box<Self>) -> String {
    let total = utils::read_atomic(&self.total_used);
    let (results, offset) = self.take();
    let rows: Vec<_> = results
      .into_iter()
      .skip(offset)
      .map(|memory| memory.serialize(total))
      .collect();

    utils::print_json(HEADERS, rows)
  }

  fn print_csv(self: Box<Self>) -> String {
    let total = utils::read_atomic(&self.total_used);
    let (results, offset) = self.take();
    let rows: Vec<_> = results
      .into_iter()
      .skip(offset)
      .map(|memory| memory.serialize(total))
      .collect();

    utils::print_csv(HEADERS, rows)
  }
}

async fn scan_node(state: &State, server: Server, client: RedisClient) -> Result<(usize, usize), RedisError> {
  let scanner = client.scan(&state.argv.pattern, Some(state.argv.page_size), None);
  let filter = state.argv.filter.as_ref().and_then(|s| Regex::new(s).ok());
  let reject = state.argv.reject.as_ref().and_then(|s| Regex::new(s).ok());
  let group = state.cmd_argv.group_by.as_ref().and_then(|s| Regex::new(s).ok());

  utils::scan_server(
    server.clone(),
    state.argv.ignore,
    state.argv.delay,
    scanner,
    move |mut scanned, mut success, mut skipped, errored, keys| {
      let (filter, reject, group, client, server) = (
        filter.clone(),
        reject.clone(),
        group.clone(),
        client.clone(),
        server.clone(),
      );

      async move {
        state.counters.incr_scanned(keys.len());
        scanned += keys.len();

        let keys: Vec<_> = keys
          .into_iter()
          .filter(|key| {
            if utils::should_skip_key_by_regexp(&filter, &reject, key) {
              skipped += 1;
              state.counters.incr_skipped(1);
              false
            } else {
              true
            }
          })
          .collect();

        if !keys.is_empty() {
          debug!("Calling MEMORY USAGE on {} keys...", keys.len());
          // if this fails in this context it's a bug
          let pipeline = client.pipeline();
          for key in keys.iter() {
            pipeline
              .memory_usage(key.clone(), state.cmd_argv.samples.clone())
              .await?;
          }

          let sizes = match pipeline.all::<Vec<Option<i64>>>().await {
            Ok(sizes) => sizes,
            Err(e) => {
              error!("{} Error calling MEMORY USAGE: {:?}", server, e);

              if state.argv.ignore {
                return Ok((scanned, success, skipped, errored));
              } else {
                return Err(e);
              }
            },
          };
          state.counters.incr_success(keys.len());
          success += keys.len();

          for (idx, key) in keys.into_iter().enumerate() {
            if let Some(memory_usage) = sizes[idx] {
              if memory_usage > 0 {
                utils::incr_atomic(&state.total_used, memory_usage as usize);
              }

              let group_captures = utils::regexp_capture(&group, &key, &state.cmd_argv.group_by_delimiter);
              if state.cmd_argv.filter_missing_groups && group.is_some() && group_captures.is_none() {
                skipped += 1;
                state.counters.incr_skipped(1);
                continue;
              }

              state.pqueue.push_or_update(Memory {
                key,
                group: group_captures,
                memory_usage,
              });
            }
          }
        }

        Ok((scanned, success, skipped, errored))
      }
    },
  )
  .await
}

pub struct MemoryCommand;

impl Command for MemoryCommand {
  fn run(
    argv: Arc<Argv>,
    _: RedisClient,
    nodes: Vec<ClusterNode>,
  ) -> impl Future<Output = Result<Option<Box<dyn Output>>, RedisError>> + Send {
    async move {
      let cmd_argv = match argv.command {
        Commands::Memory(ref inner) => Arc::new(inner.clone()),
        _ => return Err(RedisError::new(RedisErrorKind::Config, "Invalid command")),
      };

      let mut tasks = Vec::with_capacity(nodes.len());
      let counters = Counters::new();
      let max_size = cmd_argv.max_index_size.unwrap_or(cmd_argv.limit + cmd_argv.offset);
      let pqueue = Arc::new(PrioQueue::new(cmd_argv.sort.clone(), max_size as usize));
      let state = State {
        total_used: Arc::new(AtomicUsize::new(0)),
        argv: argv.clone(),
        cmd_argv,
        pqueue,
        counters,
      };

      progress::watch_totals(&state.counters);
      status!("Connecting to servers...");
      for node in nodes.into_iter() {
        let state = state.clone();
        tasks.push(tokio::spawn(async move {
          let client = node.builder.build()?;
          client.init().await?;
          utils::check_readonly(&node, &client).await?;

          let estimate: u64 = client.dbsize().await?;
          global_progress().add_server(&node.server, Some(estimate), None);
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
      status!(format!(
        "Finished ({}/{} updated, {} skipped).",
        state.counters.read_success(),
        state.counters.read_scanned(),
        state.counters.read_skipped()
      ));
      Ok(Some(Box::new(state) as Box<dyn Output>))
    }
  }
}
