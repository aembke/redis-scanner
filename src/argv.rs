use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug, Clone)]
#[command(
  version,
  about,
  long_about = "Utilities for inspecting aspects of a Redis keyspace via the SCAN command."
)]
#[command(propagate_version = true)]
pub struct Argv {
  // Shared Arguments
  /// The server hostname.
  #[arg(short = 'H', long = "host", default_value = "127.0.0.1", value_name = "STRING")]
  pub host:             String,
  /// The server port.
  #[arg(short = 'p', long = "port", default_value = "6379", value_name = "NUMBER")]
  pub port:             u16,
  /// The database to `SELECT` after connecting.
  #[arg(long = "db", value_name = "NUMBER")]
  pub db:               Option<u8>,
  /// The username to provide when authenticating.
  #[arg(short = 'u', long = "username", env = "REDIS_USERNAME", value_name = "STRING")]
  pub username:         Option<String>,
  /// The password to provide after connection.
  #[arg(short = 'P', long = "password", env = "REDIS_PASSWORD", value_name = "STRING")]
  pub password:         Option<String>,
  /// The name of the sentinel service, if using a sentinel deployment.
  #[arg(long = "sentinel-service", value_name = "STRING", conflicts_with = "cluster")]
  pub sentinel_service: Option<String>,
  /// Whether to discover other nodes in a Redis cluster.
  #[arg(short = 'c', long = "cluster", default_value = "false")]
  pub cluster:          bool,
  /// Whether to scan replicas rather than primary nodes. This also implies `--cluster`.
  #[arg(short = 'r', long = "replicas", default_value = "false")]
  pub replicas:         bool,
  /// Whether to hide progress bars and messages before the final output.
  #[arg(short = 'q', long = "quiet", default_value = "false")]
  pub quiet:            bool,
  /// Ignore errors, if possible.
  #[arg(short = 'i', long = "ignore", default_value = "false")]
  pub ignore:           bool,

  // TLS Arguments
  /// Whether to use TLS when connecting to servers.
  #[arg(long = "tls", default_value = "false")]
  pub tls:      bool,
  /// A file path to the private key for a x509 identity. .
  #[arg(long = "tls-key", value_name = "PATH")]
  pub tls_key:  Option<String>,
  /// A file path to the certificate for a x509 identity.
  #[arg(long = "tls-cert", value_name = "PATH")]
  pub tls_cert: Option<String>,

  // Shared Scan Arguments
  /// The glob pattern to provide in each `SCAN` command.
  #[arg(long = "pattern", value_name = "STRING", default_value = "*")]
  pub pattern:   String,
  /// The number of results to request in each `SCAN` command.
  #[arg(long = "page-size", default_value = "100", allow_negative_numbers = false)]
  pub page_size: u32,
  /// A delay, in milliseconds, to wait between `SCAN` commands.
  #[arg(short = 'd', long = "delay", default_value = "0")]
  pub delay:     u64,
  /// A regular expression used to filter keys while scanning. Keys that do not match will be skipped before
  /// any subsequent operations are performed.
  #[arg(short = 'f', long = "filter", value_name = "REGEXP")]
  pub filter:    Option<String>,

  // Command Arguments
  #[command(subcommand)]
  pub command: Commands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
  /// Inspect keys via the `OBJECT IDLETIME` command.
  Idle(IdleArgv),
  /// Inspect keys via the `MEMORY USAGE` command.
  Memory(MemoryArgv),
  /// Call `TOUCH` on each key.
  Touch(TouchArgv),
  /// Inspect keys via the `TTL` command.
  Ttl(TtlArgv),
}

/// The available output formats.
#[derive(ValueEnum, Clone, Debug)]
pub enum OutputFormat {
  Table,
  Csv,
  Json,
}

/// The sort order to use.
#[derive(ValueEnum, Clone, Debug)]
pub enum Sort {
  Asc,
  Desc,
}

#[derive(Args, Clone, Debug)]
pub struct IdleArgv {
  /// The output format, if applicable.
  #[arg(short = 'f', long = "format", default_value = "table", value_name = "STRING")]
  pub format: OutputFormat,
  /// The sort order to use.
  #[arg(short = 'S', long = "sort", default_value = "desc", value_name = "STRING")]
  pub sort:   Sort,
  /// The maximum number of results to return.
  #[arg(
    short = 'l',
    long = "limit",
    default_value = "100",
    allow_negative_numbers = false,
    value_name = "NUMBER"
  )]
  pub limit:  u64,
  /// The number of results to skip, after sorting. Note: the client must hold at least `limit + offset` keys in
  /// memory.
  #[arg(
    short = 'o',
    long = "offset",
    default_value = "0",
    allow_negative_numbers = false,
    value_name = "NUMBER"
  )]
  pub offset: u64,
}

#[derive(Args, Clone, Debug)]
pub struct MemoryArgv {
  /// The output format, if applicable.
  #[arg(short = 'f', long = "format", default_value = "table", value_name = "STRING")]
  pub format:   OutputFormat,
  /// The sort order to use.
  #[arg(short = 'S', long = "sort", default_value = "desc", value_name = "STRING")]
  pub sort:     Sort,
  /// A regular expression used to group or transform keys while aggregating results. This is often used to extract
  /// substrings in a key.
  #[arg(short = 'g', long = "group-by", value_name = "REGEXP")]
  pub group_by: Option<String>,
  /// The maximum number of results to return.
  #[arg(
    short = 'l',
    long = "limit",
    default_value = "100",
    allow_negative_numbers = false,
    value_name = "NUMBER"
  )]
  pub limit:    u64,
  /// The number of results to skip, after sorting. Note: the client must hold at least `limit + offset` keys in
  /// memory.
  #[arg(
    short = 'o',
    long = "offset",
    default_value = "0",
    allow_negative_numbers = false,
    value_name = "NUMBER"
  )]
  pub offset:   u64,
  /// The number of samples to provide in each `MEMORY USAGE` command.
  #[arg(
    short = 's',
    long = "samples",
    default_value = "0",
    allow_negative_numbers = false,
    value_name = "NUMBER"
  )]
  pub samples:  u64,
}

#[derive(Args, Clone, Debug)]
pub struct TouchArgv {}

#[derive(Args, Clone, Debug)]
pub struct TtlArgv {
  /// The output format, if applicable.
  #[arg(short = 'f', long = "format", default_value = "table", value_name = "STRING")]
  pub format: OutputFormat,
  /// The sort order to use.
  #[arg(short = 'S', long = "sort", default_value = "desc", value_name = "STRING")]
  pub sort:   Sort,
  /// The maximum number of results to return.
  #[arg(
    short = 'l',
    long = "limit",
    default_value = "100",
    allow_negative_numbers = false,
    value_name = "NUMBER"
  )]
  pub limit:  u64,
  /// The number of results to skip, after sorting. Note: the client must hold at least `limit + offset` keys in
  /// memory.
  #[arg(
    short = 'o',
    long = "offset",
    default_value = "0",
    allow_negative_numbers = false,
    value_name = "NUMBER"
  )]
  pub offset: u64,
}
