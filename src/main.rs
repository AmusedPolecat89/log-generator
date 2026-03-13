//! Hyper-optimized log generator CLI.
//!
//! Usage:
//!   log-generator --scenario scenario.toml --output /var/log/test.log
//!   log-generator --scenario scenario.toml --output null --metrics

use clap::Parser;
use log_generator::config::scenario::{
    RatePreset, RatePresets, RateSetting, Scenario, TimelineEvent,
};
use log_generator::generator::engine::Engine;
use log_generator::output::{HttpBatchFormat, HttpConfig, OutputConfig};
use log_generator::templates::LogFormat;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "log-generator")]
#[command(about = "Hyper-optimized log generator capable of 1GB+/s throughput")]
#[command(version)]
struct Args {
    /// Run as HTTP daemon on this port (e.g. --daemon 9090)
    #[arg(long)]
    daemon: Option<u16>,

    /// Scenario script file (TOML format)
    #[arg(short, long, required_unless_present = "daemon")]
    scenario: Option<PathBuf>,

    /// Output destination: file path, "stdout", "null", or HTTP URL (http://...)
    #[arg(short, long, default_value = "stdout")]
    output: String,

    /// HTTP batch size in KB (default: 1024 = 1MB)
    #[arg(long, default_value = "1024")]
    http_batch_kb: usize,

    /// HTTP request timeout in seconds (default: 30)
    #[arg(long, default_value = "30")]
    http_timeout: u64,

    /// HTTP authorization header (e.g., "Bearer token123")
    #[arg(long)]
    http_auth: Option<String>,

    /// Custom HTTP header (repeatable, format: "Name: Value")
    #[arg(long = "http-header", value_name = "NAME:VALUE")]
    http_headers: Vec<String>,

    /// Number of concurrent HTTP sender threads (default: 8)
    #[arg(long, default_value = "8")]
    http_senders: usize,

    /// HTTP send queue size (0 = auto = num_senders * 8)
    #[arg(long, default_value = "0")]
    http_queue_size: usize,

    /// Override log format: apache, nginx, json, syslog
    #[arg(short, long)]
    format: Option<String>,

    /// Number of worker threads (default: number of CPU cores)
    #[arg(short, long)]
    threads: Option<usize>,

    /// Show real-time throughput metrics
    #[arg(short, long)]
    metrics: bool,

    /// Override rate: preset name (trickle, low, medium, high, full, max)
    /// or numeric logs/sec with optional K/M suffix (e.g., 1000000, 500K, 1M)
    #[arg(short, long)]
    rate: Option<String>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    // Daemon mode
    if let Some(port) = args.daemon {
        log_generator::daemon::run_daemon(port);
        return;
    }

    let scenario_path = args.scenario.expect("scenario required when not in daemon mode");

    // Load scenario
    let scenario_content = match std::fs::read_to_string(&scenario_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading scenario file: {}", e);
            process::exit(1);
        }
    };

    let mut scenario: Scenario = match toml::from_str(&scenario_content) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error parsing scenario file: {}", e);
            process::exit(1);
        }
    };

    // Override format if specified
    if let Some(format_str) = &args.format {
        scenario.format = Some(match format_str.as_str() {
            "apache" => LogFormat::Apache,
            "nginx" => LogFormat::Nginx,
            "json" => LogFormat::Json,
            "syslog" => LogFormat::Syslog,
            "helios" => LogFormat::Helios,
            _ => {
                eprintln!("Unknown format: {}. Use: apache, nginx, json, syslog, helios", format_str);
                process::exit(1);
            }
        });
    }

    // Override rate if specified
    if let Some(rate_str) = &args.rate {
        let rate_setting = parse_rate_setting(rate_str, &scenario.rates);
        scenario.timeline = vec![TimelineEvent {
            at: Duration::ZERO,
            rate: rate_setting,
            duration: None,
            error_rate: None,
        }];
    }

    // Determine if using Helios format
    let is_helios = scenario.format == Some(LogFormat::Helios);

    // Configure output
    let output_config = match args.output.as_str() {
        "null" => OutputConfig::Null,
        "stdout" => OutputConfig::Stdout,
        url if url.starts_with("http://") || url.starts_with("https://") => {
            let mut http_config = HttpConfig::new(url)
                .with_batch_size(args.http_batch_kb * 1024)
                .with_timeout(Duration::from_secs(args.http_timeout))
                .with_num_senders(args.http_senders)
                .with_send_queue_size(args.http_queue_size);

            if let Some(auth) = &args.http_auth {
                http_config = http_config.with_auth(auth);
            }

            for header in &args.http_headers {
                if let Some((name, value)) = header.split_once(':') {
                    http_config = http_config.with_header(name.trim(), value.trim());
                } else {
                    eprintln!("Invalid header format '{}'. Use: 'Name: Value'", header);
                    process::exit(1);
                }
            }

            // Auto-configure batch format based on log format
            if is_helios {
                http_config = http_config
                    .with_batch_format(HttpBatchFormat::Helios)
                    .with_content_type("application/json");
            } else if scenario.format == Some(LogFormat::Json) {
                http_config = http_config
                    .with_batch_format(HttpBatchFormat::JsonArray)
                    .with_content_type("application/json");
            }

            OutputConfig::Http(http_config)
        }
        path => OutputConfig::File(PathBuf::from(path)),
    };

    // Set up graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("\nShutting down...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Create and run engine
    let num_threads = args.threads.unwrap_or_else(get_num_cpus);

    if args.verbose {
        eprintln!("Starting log generator:");
        eprintln!("  Scenario: {}", scenario_path.display());
        eprintln!("  Output: {}", args.output);
        eprintln!("  Format: {:?}", scenario.format.unwrap_or(LogFormat::Apache));
        eprintln!("  Threads: {}", num_threads);
        eprintln!("  Duration: {:?}", scenario.scenario.total_duration);
    }

    let mut engine = Engine::new(scenario, output_config, num_threads, args.metrics);

    if let Err(e) = engine.run(running) {
        eprintln!("Error running generator: {}", e);
        process::exit(1);
    }
}

fn get_num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

/// Parse a --rate value into a RateSetting.
/// Accepts preset names (trickle, low, medium, high, full, max) or numeric
/// values with optional K/M suffix (e.g., 1000000, 500K, 1M).
fn parse_rate_setting(s: &str, presets: &RatePresets) -> RateSetting {
    // Check if it's a known preset name
    if presets.get(s).is_some() {
        return RateSetting::Preset(s.to_string());
    }

    // Try to parse as numeric (with optional K/M suffix)
    let s_upper = s.to_uppercase();
    let (num_str, multiplier) = if let Some(n) = s_upper.strip_suffix('M') {
        (n, 1_000_000u64)
    } else if let Some(n) = s_upper.strip_suffix('K') {
        (n, 1_000u64)
    } else {
        (s_upper.as_str(), 1u64)
    };

    match num_str.parse::<u64>() {
        Ok(n) => {
            let lps = n * multiplier;
            let preset = RatePreset::from_logs_per_sec(lps);
            RateSetting::Explicit {
                throughput_mb: preset.throughput_mb,
                logs_per_sec: preset.logs_per_sec,
            }
        }
        Err(_) => {
            eprintln!(
                "Unknown rate '{}'. Use a preset (trickle, low, medium, high, full, max) or a number with optional K/M suffix.",
                s
            );
            process::exit(1);
        }
    }
}
