use crate::{
  argv::{Argv, TouchArgv},
  clear_status,
  progress::{global_progress, setup_event_logs, Counters, STEADY_TICK_DURATION_MS},
  status,
  utils,
  ClusterNode,
};
use fred::{
  prelude::*,
  types::{ClusterHash, CustomCommand, Scanner},
};
use futures::{future::try_join_all, TryStreamExt};
use log::{debug, error};
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;

async fn scan_node(
  argv: &Arc<Argv>,
  counters: &Arc<Counters>,
  server: Server,
  client: RedisClient,
) -> Result<(usize, usize), RedisError> {
  let touch = CustomCommand::new_static("TOUCH", ClusterHash::FirstKey, false);
  let (mut local_scanned, mut local_success) = (0, 0);
  let mut scanner = client.scan(&argv.pattern, Some(argv.page_size), None);

  let mut last_error = None;
  loop {
    let mut page = match scanner.try_next().await {
      Ok(Some(page)) => page,
      Ok(None) => break,
      Err(e) => {
        last_error = Some(e);
        break;
      },
    };

    if let Some(results) = page.take_results() {
      counters.incr_scanned(results.len());
      local_scanned += results.len();

      if !results.is_empty() {
        debug!("Calling TOUCH on {} keys...", results.len());
        let count: usize = match client.custom(touch.clone(), results).await {
          Ok(amt) => amt,
          Err(e) => {
            error!("{} Error calling TOUCH: {:?}", server, e);
            last_error = Some(e);

            if argv.ignore {
              let _ = page.next();
              continue;
            } else {
              break;
            }
          },
        };

        counters.incr_success(count);
        local_success += count;
      }
    }

    if argv.delay > 0 && page.has_more() {
      if argv.delay > STEADY_TICK_DURATION_MS {
        global_progress().update(
          &server,
          format!("Sleeping for {}ms", argv.delay),
          Some(local_scanned as u64),
        );
      }

      sleep(Duration::from_millis(argv.delay)).await;
    }

    global_progress().update(
      &server,
      format!("{} updated", local_success),
      Some(local_scanned as u64),
    );
    if let Err(e) = page.next() {
      // the more useful error shows up on the next try_next() call
      debug!("Error trying to scan next page: {:?}", e);
    }
  }

  if let Some(error) = last_error {
    global_progress().finish(&server, format!("Errored: {:?}", error));

    if argv.ignore {
      Ok((local_success, local_scanned))
    } else {
      Err(error)
    }
  } else {
    let percent = if local_scanned == 0 {
      0.0
    } else {
      local_success as f64 / local_scanned as f64 * 100.0
    };

    global_progress().finish(
      &server,
      format!(
        "Finished scanning ({}/{} = {:.2}% updated).",
        local_success, local_scanned, percent
      ),
    );
    Ok((local_success, local_scanned))
  }
}

pub async fn run(argv: &Arc<Argv>, _: &TouchArgv, _: RedisClient, nodes: Vec<ClusterNode>) -> Result<(), RedisError> {
  let mut tasks = Vec::with_capacity(nodes.len());
  let counters = Counters::new();

  status!("Connecting to servers...");
  for node in nodes.into_iter() {
    let (argv, counters) = (argv.clone(), counters.clone());
    tasks.push(tokio::spawn(async move {
      let client = node.builder.build()?;
      client.init().await?;

      let estimate: u64 = client.dbsize().await?;
      global_progress().add_server(&node.server, Some(estimate));
      let estimate_task = tokio::spawn(utils::update_estimate(node.server.clone(), client.clone()));
      let event_task = setup_event_logs(&client);

      let result = scan_node(&argv, &counters, node.server, client).await;
      estimate_task.abort();
      event_task.abort();
      result
    }));
  }
  let results = try_join_all(tasks).await?;

  clear_status!();
  if let Some(err) = results.into_iter().find_map(|e| e.err()) {
    eprintln!("Fatal error while scanning: {:?}", err);
  }

  status!(format!(
    "Finished ({}/{} updated).",
    counters.read_success(),
    counters.read_scanned()
  ));
  Ok(())
}
