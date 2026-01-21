//! Hyper-optimized log generator CLI.
//!
//! Usage:
//!   log-generator --scenario scenario.toml --output /var/log/test.log
//!   log-generator --scenario scenario.toml --output null --metrics

use clap::Parser;
use log_generator::config::scenario::Scenario;
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
    /// Scenario script file (TOML format)
    #[arg(short, long)]
    scenario: PathBuf,

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

    /// Override log format: apache, nginx, json, syslog
    #[arg(short, long)]
    format: Option<String>,

    /// Number of worker threads (default: number of CPU cores)
    #[arg(short, long)]
    threads: Option<usize>,

    /// Show real-time throughput metrics
    #[arg(short, long)]
    metrics: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    // Load scenario
    let scenario_content = match std::fs::read_to_string(&args.scenario) {
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

    // Determine if using Helios format
    let is_helios = scenario.format == Some(LogFormat::Helios);

    // Configure output
    let output_config = match args.output.as_str() {
        "null" => OutputConfig::Null,
        "stdout" => OutputConfig::Stdout,
        url if url.starts_with("http://") || url.starts_with("https://") => {
            let mut http_config = HttpConfig::new(url)
                .with_batch_size(args.http_batch_kb * 1024)
                .with_timeout(Duration::from_secs(args.http_timeout));

            if let Some(auth) = &args.http_auth {
                http_config = http_config.with_auth(auth);
            }

            // Auto-configure for Helios API when using helios format
            if is_helios {
                http_config = http_config
                    .with_batch_format(HttpBatchFormat::Helios)
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
        eprintln!("  Scenario: {}", args.scenario.display());
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
