//! HTTP daemon for remote control of the log generator.
//!
//! Exposes a simple REST API:
//!   POST /start  — start a generation run (JSON body with config)
//!   POST /stop   — stop the current run
//!   GET  /status — get current state

use crate::config::scenario::{RateSetting, Scenario, ScenarioMeta, TimelineEvent};
use crate::generator::engine::Engine;
use crate::output::{HttpBatchFormat, HttpConfig, OutputConfig};
use crate::templates::LogFormat;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// State shared between the HTTP handler and the generator thread.
struct RunState {
    running: Arc<AtomicBool>,
    started_at: Instant,
    config: StartRequest,
}

/// JSON body for POST /start
#[derive(Debug, Clone, Deserialize)]
pub struct StartRequest {
    /// Output URL (required, e.g. "http://host:8080/api/v1/ingest")
    pub output: String,

    /// Duration string (e.g. "15m", "1h", "30s")
    pub duration: String,

    /// Log format: "apache", "nginx", "json", "syslog", "helios"
    #[serde(default = "default_format")]
    pub format: String,

    /// Rate: preset name ("trickle","low","medium","high","full","max")
    /// or explicit { throughput_mb, logs_per_sec }
    #[serde(default)]
    pub rate: Option<RateRequest>,

    /// HTTP auth header value (e.g. "Bearer token123")
    #[serde(default)]
    pub http_auth: Option<String>,

    /// Custom HTTP headers as key-value pairs
    #[serde(default)]
    pub http_headers: Vec<HeaderPair>,

    /// HTTP batch size in KB (default: 1024)
    #[serde(default = "default_batch_kb")]
    pub http_batch_kb: usize,

    /// HTTP timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub http_timeout: u64,

    /// Number of HTTP sender threads (default: 8)
    #[serde(default = "default_senders")]
    pub http_senders: usize,

    /// Number of worker threads (default: num CPUs)
    #[serde(default)]
    pub threads: Option<usize>,

    /// Inline scenario TOML (alternative to building one from fields)
    #[serde(default)]
    pub scenario_toml: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RateRequest {
    Preset(String),
    Explicit { throughput_mb: u32, logs_per_sec: u64 },
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeaderPair {
    pub name: String,
    pub value: String,
}

fn default_format() -> String {
    "helios".to_string()
}
fn default_batch_kb() -> usize {
    1024
}
fn default_timeout() -> u64 {
    30
}
fn default_senders() -> usize {
    8
}

/// JSON response for GET /status
#[derive(Serialize)]
struct StatusResponse {
    state: &'static str,
    uptime_secs: Option<f64>,
    config: Option<StatusConfig>,
}

#[derive(Serialize)]
struct StatusConfig {
    output: String,
    duration: String,
    format: String,
}

/// Run the daemon HTTP server on the given port.
pub fn run_daemon(port: u16) {
    let addr = format!("0.0.0.0:{}", port);
    let server = match tiny_http::Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to bind to {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    eprintln!("Daemon listening on http://0.0.0.0:{}", port);
    eprintln!("Endpoints:");
    eprintln!("  POST /start  — start generation (JSON body)");
    eprintln!("  POST /stop   — stop current run");
    eprintln!("  GET  /status — get current state");

    let current_run: Arc<Mutex<Option<RunState>>> = Arc::new(Mutex::new(None));

    // Handle Ctrl-C to shut down the daemon itself
    let daemon_running = Arc::new(AtomicBool::new(true));
    let dr = daemon_running.clone();
    let run_ref = Arc::clone(&current_run);
    ctrlc::set_handler(move || {
        eprintln!("\nShutting down daemon...");
        // Stop any active run
        if let Ok(guard) = run_ref.lock() {
            if let Some(ref state) = *guard {
                state.running.store(false, Ordering::SeqCst);
            }
        }
        dr.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    while daemon_running.load(Ordering::SeqCst) {
        // Use recv_timeout so we can check the shutdown flag
        let request = match server.recv_timeout(Duration::from_millis(500)) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(_) => continue,
        };

        let method = request.method().to_string();
        let url = request.url().to_string();

        match (method.as_str(), url.as_str()) {
            ("POST", "/start") => handle_start(request, &current_run),
            ("POST", "/stop") => handle_stop(request, &current_run),
            ("GET", "/status") => handle_status(request, &current_run),
            _ => {
                let resp = tiny_http::Response::from_string(
                    r#"{"error":"not found","endpoints":["POST /start","POST /stop","GET /status"]}"#,
                )
                .with_status_code(404)
                .with_header(
                    "Content-Type: application/json"
                        .parse::<tiny_http::Header>()
                        .unwrap(),
                );
                let _ = request.respond(resp);
            }
        }
    }
}

fn json_response(
    request: tiny_http::Request,
    status: u16,
    body: &str,
) {
    let resp = tiny_http::Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(
            "Content-Type: application/json"
                .parse::<tiny_http::Header>()
                .unwrap(),
        );
    let _ = request.respond(resp);
}

fn handle_start(
    mut request: tiny_http::Request,
    current_run: &Arc<Mutex<Option<RunState>>>,
) {
    // Check if already running
    {
        let guard = current_run.lock().unwrap();
        if let Some(ref state) = *guard {
            if state.running.load(Ordering::SeqCst) {
                json_response(
                    request,
                    409,
                    r#"{"error":"generator already running, POST /stop first"}"#,
                );
                return;
            }
        }
    }

    // Read request body
    let mut body = String::new();
    if let Err(e) = request.as_reader().read_to_string(&mut body) {
        json_response(request, 400, &format!(r#"{{"error":"failed to read body: {}"}}"#, e));
        return;
    }

    let config: StartRequest = match serde_json::from_str(&body) {
        Ok(c) => c,
        Err(e) => {
            json_response(request, 400, &format!(r#"{{"error":"invalid JSON: {}"}}"#, e));
            return;
        }
    };

    // Build scenario
    let scenario = match build_scenario(&config) {
        Ok(s) => s,
        Err(e) => {
            json_response(request, 400, &format!(r#"{{"error":"{}"}}"#, e));
            return;
        }
    };

    // Build output config
    let output_config = match build_output_config(&config) {
        Ok(c) => c,
        Err(e) => {
            json_response(request, 400, &format!(r#"{{"error":"{}"}}"#, e));
            return;
        }
    };

    let num_threads = config.threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4)
    });

    let running = Arc::new(AtomicBool::new(true));
    let run_state = RunState {
        running: Arc::clone(&running),
        started_at: Instant::now(),
        config: config.clone(),
    };

    // Store state
    {
        let mut guard = current_run.lock().unwrap();
        *guard = Some(run_state);
    }

    let duration_str = config.duration.clone();
    let format_str = config.format.clone();
    let output_str = config.output.clone();
    let current_run_ref = Arc::clone(current_run);

    // Spawn generator thread
    std::thread::Builder::new()
        .name("generator-run".to_string())
        .spawn(move || {
            eprintln!("Starting generator: format={}, duration={}, output={}", format_str, duration_str, output_str);
            let mut engine = Engine::new(scenario, output_config, num_threads, true);
            match engine.run(running) {
                Ok(()) => eprintln!("Generator run completed successfully."),
                Err(e) => eprintln!("Generator run failed: {}", e),
            }
            // Clear state
            if let Ok(mut guard) = current_run_ref.lock() {
                *guard = None;
            }
        })
        .expect("Failed to spawn generator thread");

    json_response(request, 200, r#"{"status":"started"}"#);
}

fn handle_stop(
    request: tiny_http::Request,
    current_run: &Arc<Mutex<Option<RunState>>>,
) {
    let guard = current_run.lock().unwrap();
    match *guard {
        Some(ref state) if state.running.load(Ordering::SeqCst) => {
            state.running.store(false, Ordering::SeqCst);
            json_response(request, 200, r#"{"status":"stopping"}"#);
        }
        _ => {
            json_response(request, 200, r#"{"status":"idle","message":"nothing to stop"}"#);
        }
    }
}

fn handle_status(
    request: tiny_http::Request,
    current_run: &Arc<Mutex<Option<RunState>>>,
) {
    let guard = current_run.lock().unwrap();
    let response = match *guard {
        Some(ref state) if state.running.load(Ordering::SeqCst) => {
            let uptime = state.started_at.elapsed().as_secs_f64();
            StatusResponse {
                state: "running",
                uptime_secs: Some(uptime),
                config: Some(StatusConfig {
                    output: state.config.output.clone(),
                    duration: state.config.duration.clone(),
                    format: state.config.format.clone(),
                }),
            }
        }
        _ => StatusResponse {
            state: "idle",
            uptime_secs: None,
            config: None,
        },
    };
    let body = serde_json::to_string(&response).unwrap();
    json_response(request, 200, &body);
}

fn parse_format(s: &str) -> Result<LogFormat, String> {
    match s {
        "apache" => Ok(LogFormat::Apache),
        "nginx" => Ok(LogFormat::Nginx),
        "json" => Ok(LogFormat::Json),
        "syslog" => Ok(LogFormat::Syslog),
        "helios" => Ok(LogFormat::Helios),
        _ => Err(format!("unknown format '{}', use: apache, nginx, json, syslog, helios", s)),
    }
}

fn build_scenario(config: &StartRequest) -> Result<Scenario, String> {
    // If inline TOML is provided, use it directly
    if let Some(ref toml_str) = config.scenario_toml {
        let mut scenario: Scenario = toml::from_str(toml_str)
            .map_err(|e| format!("invalid scenario TOML: {}", e))?;
        let format = parse_format(&config.format)?;
        scenario.format = Some(format);
        return Ok(scenario);
    }

    let duration = crate::config::scenario::parse_duration(&config.duration)
        .map_err(|e| format!("invalid duration '{}': {}", config.duration, e))?;

    let format = parse_format(&config.format)?;

    let rate_setting = match &config.rate {
        Some(RateRequest::Preset(name)) => RateSetting::Preset(name.clone()),
        Some(RateRequest::Explicit { throughput_mb, logs_per_sec }) => RateSetting::Explicit {
            throughput_mb: *throughput_mb,
            logs_per_sec: *logs_per_sec,
        },
        None => {
            // Default to low (100 MB/s)
            RateSetting::Preset("low".to_string())
        }
    };

    Ok(Scenario {
        scenario: ScenarioMeta {
            name: "remote".to_string(),
            total_duration: duration,
        },
        timeline: vec![TimelineEvent {
            at: Duration::ZERO,
            rate: rate_setting,
            duration: None,
            error_rate: None,
        }],
        spikes: vec![],
        rates: Default::default(),
        format: Some(format),
    })
}

fn build_output_config(config: &StartRequest) -> Result<OutputConfig, String> {
    let url = &config.output;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("output must be an HTTP URL, got '{}'", url));
    }

    let format = parse_format(&config.format)?;
    let is_helios = format == LogFormat::Helios;

    let mut http_config = HttpConfig::new(url)
        .with_batch_size(config.http_batch_kb * 1024)
        .with_timeout(Duration::from_secs(config.http_timeout))
        .with_num_senders(config.http_senders);

    if let Some(ref auth) = config.http_auth {
        http_config = http_config.with_auth(auth);
    }

    for header in &config.http_headers {
        http_config = http_config.with_header(&header.name, &header.value);
    }

    if is_helios {
        http_config = http_config
            .with_batch_format(HttpBatchFormat::Helios)
            .with_content_type("application/json");
    } else if format == LogFormat::Json {
        http_config = http_config
            .with_batch_format(HttpBatchFormat::JsonArray)
            .with_content_type("application/json");
    }

    Ok(OutputConfig::Http(http_config))
}
