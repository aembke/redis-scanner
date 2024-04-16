use crate::utils;
use fred::{
  clients::RedisClient,
  interfaces::{ClientLike, EventInterface},
  types::Server,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::debug;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::{
  borrow::Cow,
  collections::HashMap,
  sync::{atomic::AtomicUsize, Arc},
  time::Duration as StdDuration,
};
use tokio::task::JoinHandle;

pub const STEADY_TICK_DURATION_MS: u64 = 150;
const SPINNER_BAR_STYLE_TEMPLATE: &str = "[{elapsed_precise}] {prefix:.bold} {spinner} {msg}";
const COUNTER_BAR_STYLE_TEMPLATE: &str = "[{elapsed_precise}] {prefix:.bold} {bar:40} {pos}/{len} {msg}";
const STATUS_BAR_STYLE_TEMPLATE: &str = "{prefix:.bold} {wide_msg}";
const QUIET_OUTPUT: AtomicUsize = AtomicUsize::new(0);
static PROGRESS: Lazy<Progress> = Lazy::new(|| Progress::default());

pub fn global_progress() -> &'static Progress {
  &*PROGRESS
}

pub fn quiet_output() -> bool {
  utils::read_atomic(&QUIET_OUTPUT) != 0
}

pub fn set_quiet_output(val: bool) {
  let val: usize = if val { 1 } else { 0 };
  utils::set_atomic(&QUIET_OUTPUT, val);
}

/// Shared operation counters.
pub struct Counters {
  pub scanned: AtomicUsize,
  pub skipped: AtomicUsize,
  pub errored: AtomicUsize,
  pub success: AtomicUsize,
}

impl Counters {
  pub fn new() -> Arc<Self> {
    Arc::new(Counters {
      scanned: AtomicUsize::new(0),
      skipped: AtomicUsize::new(0),
      errored: AtomicUsize::new(0),
      success: AtomicUsize::new(0),
    })
  }

  pub fn incr_scanned(&self, amt: usize) -> usize {
    utils::incr_atomic(&self.scanned, amt)
  }

  pub fn incr_skipped(&self, amt: usize) -> usize {
    utils::incr_atomic(&self.skipped, amt)
  }

  pub fn incr_errored(&self, amt: usize) -> usize {
    utils::incr_atomic(&self.errored, amt)
  }

  pub fn incr_success(&self, amt: usize) -> usize {
    utils::incr_atomic(&self.success, amt)
  }

  pub fn set_scanned(&self, amt: usize) -> usize {
    utils::set_atomic(&self.scanned, amt)
  }

  pub fn set_skipped(&self, amt: usize) -> usize {
    utils::set_atomic(&self.skipped, amt)
  }

  pub fn set_errored(&self, amt: usize) -> usize {
    utils::set_atomic(&self.errored, amt)
  }

  pub fn set_success(&self, amt: usize) -> usize {
    utils::set_atomic(&self.success, amt)
  }

  pub fn read_scanned(&self) -> usize {
    utils::read_atomic(&self.scanned)
  }

  pub fn read_skipped(&self) -> usize {
    utils::read_atomic(&self.skipped)
  }

  pub fn read_errored(&self) -> usize {
    utils::read_atomic(&self.errored)
  }

  pub fn read_success(&self) -> usize {
    utils::read_atomic(&self.success)
  }
}

#[macro_export]
macro_rules! check_quiet {
  () => {
    if crate::progress::quiet_output() {
      return;
    }
  };
}

#[macro_export]
macro_rules! status {
  ($msg:expr) => {
    crate::progress::global_progress().status.set_message($msg);
  };
}

#[macro_export]
macro_rules! clear_status {
  () => {
    crate::progress::global_progress().status.set_message("");
  };
}

pub struct Progress {
  pub multi:  MultiProgress,
  pub bars:   Mutex<HashMap<Server, ProgressBar>>,
  pub status: ProgressBar,
}

impl Default for Progress {
  fn default() -> Self {
    let multi = MultiProgress::new();
    let bars = Mutex::new(HashMap::new());

    let status_style = ProgressStyle::with_template(STATUS_BAR_STYLE_TEMPLATE).expect("Failed to create status bar");
    let status = multi.add(ProgressBar::new_spinner());
    status.enable_steady_tick(StdDuration::from_millis(STEADY_TICK_DURATION_MS));
    status.set_style(status_style);

    Progress { multi, bars, status }
  }
}

impl Progress {
  ///
  pub fn add_server(&self, server: &Server, estimate: Option<u64>) {
    check_quiet!();

    let style = if estimate.is_some() {
      ProgressStyle::with_template(COUNTER_BAR_STYLE_TEMPLATE).expect("Failed to create counter template")
    } else {
      ProgressStyle::with_template(SPINNER_BAR_STYLE_TEMPLATE).expect("Failed to create spinner template")
    };
    let bar = if let Some(est) = estimate {
      self.multi.insert_before(&self.status, ProgressBar::new(est))
    } else {
      self.multi.insert_before(&self.status, ProgressBar::new_spinner())
    };

    bar.set_prefix(format!("{}", server));
    bar.enable_steady_tick(StdDuration::from_millis(STEADY_TICK_DURATION_MS));
    bar.set_style(style);
    self.bars.lock().insert(server.clone(), bar);
  }

  ///
  pub fn update_estimate(&self, server: &Server, estimate: u64) {
    if let Some(bar) = self.bars.lock().get_mut(server) {
      bar.set_length(estimate);
    }
  }

  pub fn remove_server(&self, server: &Server) {
    if let Some(bar) = self.bars.lock().remove(server) {
      self.multi.remove(&bar);
      bar.finish_and_clear();
    }
  }

  pub fn update(&self, server: &Server, message: impl Into<Cow<'static, str>>, position: Option<u64>) {
    check_quiet!();

    if let Some(bar) = self.bars.lock().get(server) {
      if let Some(pos) = position {
        if bar.length().is_some() {
          bar.set_position(pos);
        }
      }

      bar.set_message(message);
    }
  }

  pub fn finish(&self, server: &Server, message: impl Into<Cow<'static, str>>) {
    check_quiet!();

    if let Some(bar) = self.bars.lock().get(server) {
      bar.finish_with_message(message);
    }
  }

  pub fn clear(&self) {
    for (_, bar) in self.bars.lock().drain() {
      bar.finish_and_clear();
    }
    if let Err(e) = self.multi.clear() {
      debug!("Error clearing progress bars: {:?}", e);
    }
  }
}

pub fn setup_event_logs(client: &RedisClient) -> JoinHandle<()> {
  let client = client.clone();

  tokio::spawn(async move {
    // these clients always use centralized configs to specific servers
    let server = client
      .client_config()
      .server
      .hosts()
      .pop()
      .expect("Failed to read centralized config");

    let mut errors = client.error_rx();
    let mut reconnections = client.reconnect_rx();
    let mut unresponsive = client.unresponsive_rx();

    loop {
      tokio::select! {
        e = errors.recv() => {
          global_progress().update(&server, format!("Disconnected: {:?}", e), None);
        },
        _ = reconnections.recv() => {
          global_progress().update(&server, "Reconnected", None);
        },
        _ = unresponsive.recv() => {
          global_progress().update(&server, "Unresponsive connection.", None);
        }
      }
    }
  })
}
