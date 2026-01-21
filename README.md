# log-generator

A hyper-optimized log generator written in Rust, capable of producing **2.5+ GB/s** of realistic log data with runtime-configurable anomaly patterns.

## Performance

| Format | Throughput | Logs/Second |
|--------|------------|-------------|
| Apache Combined | 2.47 GB/s | 12.3M logs/sec |
| Nginx | 2.3 GB/s | 10.8M logs/sec |
| JSON | 2.0 GB/s | 7.5M logs/sec |
| Syslog RFC5424 | 2.2 GB/s | 11.0M logs/sec |

*Benchmarked on a free-tier GitHub Codespace (2 cores, 8GB RAM)*

## Features

- **Extreme throughput**: 2.5+ GB/s log generation (250% above 1 GB/s target)
- **Multiple log formats**: Apache Combined, Nginx, JSON structured, Syslog RFC5424
- **Scenario scripting**: Highly expressive TOML-based configuration for complex test scenarios
- **Anomaly injection**: Configurable error spikes, latency anomalies, and unusual patterns
- **Zero-allocation hot path**: Pre-generated field pools eliminate allocations during generation
- **Multi-threaded**: Scales linearly with CPU cores
- **Real-time metrics**: Live throughput and scenario state display

## Installation

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Linux (optimized for Linux-specific features)

### Build from source

```bash
git clone https://github.com/your-org/log-generator.git
cd log-generator
cargo build --release
```

The binary will be at `target/release/log-generator`.

## Quick Start

### Benchmark Mode

Run at maximum speed with output discarded:

```bash
# Create a benchmark scenario
cat > benchmark.toml << 'EOF'
[scenario]
name = "benchmark"
total_duration = "10s"

[[timeline]]
at = "0s"
rate = "full"
error_rate = 1.0
EOF

# Run benchmark
./target/release/log-generator --scenario benchmark.toml --output null --metrics
```

### Generate Log Files

```bash
# Generate Apache logs
./target/release/log-generator --scenario benchmark.toml --output /var/log/test.log

# Generate JSON logs
./target/release/log-generator --scenario benchmark.toml --format json --output logs.json

# Output to stdout
./target/release/log-generator --scenario benchmark.toml --output stdout
```

### Send to HTTP Endpoint

Stream logs to a log aggregation service or custom endpoint:

```bash
# Basic HTTP POST
./target/release/log-generator --scenario benchmark.toml \
  --output http://localhost:8080/logs \
  --format json

# With authentication and custom batch size
./target/release/log-generator --scenario benchmark.toml \
  --output https://logs.example.com/ingest \
  --format json \
  --http-auth "Bearer token123" \
  --http-batch-kb 512 \
  --http-timeout 60
```

HTTP output features:
- Automatic batching (default 1MB batches)
- Retry with exponential backoff (3 retries)
- Gzip compression support
- Connection pooling for performance
- Content-Type: `application/x-ndjson` (newline-delimited JSON)

## CLI Reference

```
USAGE:
    log-generator [OPTIONS] --scenario <FILE>

OPTIONS:
    -s, --scenario <FILE>     Scenario script file (required)
    -o, --output <PATH>       Output destination:
                              - File path: /var/log/test.log
                              - "stdout": Write to stdout
                              - "null": Discard output (benchmark mode)
                              - HTTP URL: http://host:port/path
    -f, --format <FORMAT>     Override log format: apache, nginx, json, syslog
    -t, --threads <N>         Worker threads (default: number of CPU cores)
    -m, --metrics             Show real-time throughput metrics
    -v, --verbose             Verbose output
    -h, --help                Print help information

HTTP OPTIONS:
    --http-batch-kb <SIZE>    Batch size in KB before sending (default: 1024 = 1MB)
    --http-timeout <SECS>     Request timeout in seconds (default: 30)
    --http-auth <HEADER>      Authorization header (e.g., "Bearer token123")
```

## Scenario Configuration

Scenarios are defined in TOML files with a powerful DSL for controlling generation rate, timing, and anomaly injection.

### Basic Structure

```toml
[scenario]
name = "my_scenario"
total_duration = "10m"      # Total run time

[[timeline]]                # Rate change events
at = "0s"
rate = "low"
error_rate = 0.5            # 0.5% errors

[[spikes]]                  # Anomaly injections
at = "2m"
type = "error_burst"
duration = "5s"
error_rate = 50.0           # 50% errors during spike
```

### Timeline Events

Timeline events control the base generation rate at specific points in time:

```toml
[[timeline]]
at = "0s"                   # When this rate takes effect
rate = "low"                # Rate preset: "low", "medium", "full"
error_rate = 1.0            # Base error rate (percentage)

[[timeline]]
at = "1m"
rate = "ramp_to_full"       # Gradual ramp up
duration = "30s"            # Ramp duration

[[timeline]]
at = "1m30s"
rate = "full"               # Maximum throughput
```

### Rate Presets

| Preset | Throughput | Logs/Second |
|--------|------------|-------------|
| `low` | ~100 MB/s | ~500K logs/sec |
| `medium` | ~500 MB/s | ~2.5M logs/sec |
| `full` | ~1+ GB/s | ~5M+ logs/sec |

Custom rate presets can be defined:

```toml
[rates]
low = { throughput_mb = 100, logs_per_sec = 500000 }
medium = { throughput_mb = 500, logs_per_sec = 2500000 }
full = { throughput_mb = 1000, logs_per_sec = 5000000 }
```

### Spike Types

Spikes inject anomalies at specific times:

#### Error Burst
Sudden increase in error responses (4xx/5xx status codes):

```toml
[[spikes]]
at = "2m"
type = "error_burst"
duration = "5s"
error_rate = 50.0           # 50% of responses will be errors
```

#### Latency Spike
Simulates slow responses:

```toml
[[spikes]]
at = "3m"
type = "latency_spike"
duration = "10s"
latency_multiplier = 10.0   # 10x normal latency in logs
```

#### Mixed Anomaly
Combines error burst and latency spike:

```toml
[[spikes]]
at = "4m"
type = "mixed"
duration = "8s"
error_rate = 30.0
latency_multiplier = 5.0
```

#### Unusual Patterns
Generates logs with unusual characteristics:

```toml
[[spikes]]
at = "5m"
type = "unusual_patterns"
duration = "15s"
```

### Repeated Spikes

Create multiple spikes with a single definition:

```toml
[[spikes]]
at = "2m"
type = "error_burst"
duration = "3s"
error_rate = { min = 30.0, max = 90.0 }  # Random rate per spike

[spikes.repeat]
count = 10                  # Number of spikes
interval = "20s"            # Time between spikes
jitter = "5s"               # Random timing variation (+/- 5s)
```

### Duration Format

Durations support multiple units:

- `30s` - 30 seconds
- `5m` - 5 minutes
- `2h` - 2 hours
- `1m30s` - 1 minute 30 seconds

## Log Formats

### Apache Combined

Standard Apache Combined Log Format:

```
192.168.45.123 - - [20/Jan/2026:14:30:45 +0000] "GET /api/v1/users/list HTTP/1.1" 200 1234 "https://example.com/" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0"
```

Fields:
- Client IP address
- Remote logname (always `-`)
- Remote user (always `-`)
- Timestamp in CLF format
- Request line (method, path, protocol)
- HTTP status code
- Response size in bytes
- Referer header
- User-Agent header

### Nginx

Nginx access log format with additional timing:

```
192.168.45.123 - - [20/Jan/2026:14:30:45 +0000] "GET /api/v1/users/list HTTP/1.1" 200 1234 "https://example.com/" "Mozilla/5.0..." 0.045 0.043
```

Additional fields:
- Request time (seconds)
- Upstream response time (seconds)

### JSON Structured

Modern structured logging format:

```json
{"timestamp":"2026-01-20T14:30:45.123Z","level":"INFO","service":"web-api","trace_id":"abc123def456","span_id":"789xyz","method":"GET","path":"/api/v1/users","status":200,"duration_ms":45,"client_ip":"192.168.45.123","user_agent":"Mozilla/5.0...","request_id":"req-12345"}
```

Fields:
- ISO 8601 timestamp with milliseconds
- Log level (INFO, WARN, ERROR based on status)
- Service name
- Trace ID and Span ID (for distributed tracing)
- HTTP method and path
- Status code
- Duration in milliseconds
- Client IP
- User-Agent
- Request ID

### Syslog RFC5424

Standard syslog format:

```
<134>1 2026-01-20T14:30:45.123Z webserver01 nginx 12345 ID47 - 192.168.45.123 "GET /api/v1/users HTTP/1.1" 200 1234
```

Fields:
- Priority (facility + severity)
- Version
- ISO 8601 timestamp
- Hostname
- App name
- Process ID
- Message ID
- Structured data (optional)
- Message content

## Field Generation

All fields are pre-generated at startup for zero-allocation performance:

### IP Addresses (65,536 pre-generated)

Distribution:
- 30% Private 10.x.x.x
- 20% Private 192.168.x.x
- 10% Private 172.16-31.x.x
- 40% Public IPs (avoiding reserved ranges)

### URL Paths (4,096 pre-generated)

Distribution:
- 40% API paths with IDs (`/api/v1/users/12345`)
- 20% Static resources (`/static/abc123.js`)
- 15% UUID-based paths (`/api/v1/orders/550e8400-e29b-41d4-a716-446655440000`)
- 15% Query parameters (`/api/v1/search?page=1&limit=20`)
- 10% Root/simple paths (`/`, `/health`, `/favicon.ico`)

### User Agents (256 pre-generated)

Distribution:
- 50% Chrome (desktop)
- 15% Firefox (desktop)
- 10% Safari/Edge (desktop)
- 15% Mobile (iOS/Android)
- 5% Bots (Googlebot, Bingbot, etc.)
- 5% API clients (curl, Python requests, etc.)

### HTTP Status Codes

Distribution varies by error_rate setting:
- Success (2xx): 200, 201, 204
- Redirect (3xx): 301, 302, 304
- Client Error (4xx): 400, 401, 403, 404
- Server Error (5xx): 500, 502, 503, 504

### Response Sizes

Realistic distribution based on content type:
- HTML pages: 2KB - 50KB
- JSON responses: 100B - 10KB
- Static assets: 1KB - 500KB
- Error responses: 100B - 1KB

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Main Thread                               │
│  - Scenario execution and timing                                │
│  - Metrics collection and display                               │
│  - Graceful shutdown handling (Ctrl+C)                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                    Shared Scenario State
                    (lock-free atomics)
                              │
┌──────────────┬──────────────┬──────────────┬──────────────┐
│  Worker 0    │  Worker 1    │  Worker 2    │  Worker N    │
│  (Core 0)    │  (Core 1)    │  (Core 2)    │  (Core N)    │
│ - Local RNG  │ - Local RNG  │ - Local RNG  │ - Local RNG  │
│ - Local Buf  │ - Local Buf  │ - Local Buf  │ - Local Buf  │
│ - Timestamp  │ - Timestamp  │ - Timestamp  │ - Timestamp  │
│   cache      │   cache      │   cache      │   cache      │
└──────────────┴──────────────┴──────────────┴──────────────┘
                              │
                    Crossbeam bounded channel
                    (10K log batches)
                              │
┌─────────────────────────────────────────────────────────────────┐
│                      Output Thread                               │
│  - Buffered writes (64KB buffers)                               │
│  - File / stdout / null output                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Performance Optimizations

1. **Pre-allocated field pools**: 65K IPs, 4K paths, 256 user agents generated at startup
2. **Fast integer formatting**: `itoa` crate (10-20x faster than std::fmt)
3. **Fast float formatting**: `ryu` crate for duration/timing fields
4. **Cached timestamps**: Updated every 50ms instead of per-log
5. **Lock-free state sharing**: Atomic operations for scenario state
6. **Core affinity**: Workers pinned to specific CPU cores
7. **Batched I/O**: 10,000 logs per batch, 64KB write buffers
8. **Zero allocations**: Hot path uses only pre-allocated memory

## Example Scenarios

### Simple Benchmark

```toml
[scenario]
name = "benchmark"
total_duration = "10s"

[[timeline]]
at = "0s"
rate = "full"
error_rate = 1.0
```

### Production Simulation

```toml
[scenario]
name = "production_simulation"
total_duration = "10m"

# Start with low traffic
[[timeline]]
at = "0s"
rate = "low"
error_rate = 0.5

# Morning ramp up
[[timeline]]
at = "1m"
rate = "ramp_to_full"
duration = "30s"

# Peak traffic
[[timeline]]
at = "1m30s"
rate = "full"
error_rate = 1.0

# Afternoon slowdown
[[timeline]]
at = "5m30s"
rate = "medium"
error_rate = 2.0

# Error spikes during peak
[[spikes]]
at = "2m"
type = "error_burst"
duration = "5s"
error_rate = 50.0

[[spikes]]
at = "3m"
type = "latency_spike"
duration = "10s"
latency_multiplier = 10.0

[[spikes]]
at = "4m"
type = "mixed"
duration = "8s"
error_rate = 40.0
latency_multiplier = 5.0
```

### Chaos Testing

```toml
[scenario]
name = "chaos_test"
total_duration = "5m"

[[timeline]]
at = "0s"
rate = "full"
error_rate = 1.0

# Random error spikes throughout
[[spikes]]
at = "30s"
type = "error_burst"
duration = "3s"
error_rate = { min = 20.0, max = 80.0 }

[spikes.repeat]
count = 15
interval = "15s"
jitter = "5s"
```

## Use Cases

- **Log pipeline testing**: Verify log ingestion systems can handle production-scale throughput
- **SIEM stress testing**: Test security monitoring systems under load
- **Anomaly detection validation**: Verify ML models detect injected anomalies
- **Capacity planning**: Determine storage and processing requirements
- **Performance benchmarking**: Compare log processing tools and configurations

## Dependencies

| Crate | Purpose |
|-------|---------|
| `crossbeam` | Lock-free channels for worker communication |
| `fastrand` | Fast thread-local random number generation |
| `itoa` | Fast integer-to-string conversion |
| `ryu` | Fast float-to-string conversion |
| `clap` | Command-line argument parsing |
| `serde` | Configuration serialization |
| `toml` | TOML configuration parsing |
| `core_affinity` | CPU core pinning |
| `ctrlc` | Graceful shutdown handling |

## License

MIT License - see LICENSE file for details.
