//! Concurrent HTTP endpoint output writer.
//!
//! Sends log batches to an HTTP endpoint using a pool of sender threads
//! for concurrent requests, allowing much higher throughput than a single
//! blocking sender.

use super::http::{HttpBatchFormat, HttpConfig};
use super::OutputWriter;
use crossbeam::channel::{bounded, Sender};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::io::{self, Error, ErrorKind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Channel buffer size (number of prepared payloads waiting to be sent).
const SEND_QUEUE_SIZE: usize = 32;

/// Concurrent HTTP output writer that distributes sends across a thread pool.
pub struct ConcurrentHttpWriter {
    config: HttpConfig,
    buffer: Vec<u8>,
    tx: Option<Sender<Vec<u8>>>,
    sender_threads: Vec<JoinHandle<()>>,
    bytes_written: Arc<AtomicU64>,
    requests_sent: Arc<AtomicU64>,
    requests_failed: Arc<AtomicU64>,
}

impl ConcurrentHttpWriter {
    /// Create a new concurrent HTTP writer.
    pub fn new(config: HttpConfig) -> io::Result<Self> {
        let num_senders = config.num_senders.max(1);
        let (tx, rx) = bounded::<Vec<u8>>(SEND_QUEUE_SIZE);

        let bytes_written = Arc::new(AtomicU64::new(0));
        let requests_sent = Arc::new(AtomicU64::new(0));
        let requests_failed = Arc::new(AtomicU64::new(0));

        let mut sender_threads = Vec::with_capacity(num_senders);

        for id in 0..num_senders {
            let rx = rx.clone();
            let url = config.url.clone();
            let timeout = config.timeout;
            let max_retries = config.max_retries;
            let bytes_written = Arc::clone(&bytes_written);
            let requests_sent = Arc::clone(&requests_sent);
            let requests_failed = Arc::clone(&requests_failed);

            // Build per-thread client with headers
            let mut headers = HeaderMap::new();
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_str(&config.content_type)
                    .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?,
            );
            if let Some(auth) = &config.auth_header {
                headers.insert(
                    "Authorization",
                    HeaderValue::from_str(auth)
                        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?,
                );
            }

            for (name, value) in &config.custom_headers {
                headers.insert(
                    reqwest::header::HeaderName::from_bytes(name.as_bytes())
                        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?,
                    HeaderValue::from_str(value)
                        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?,
                );
            }

            let client = Client::builder()
                .timeout(timeout)
                .default_headers(headers)
                .gzip(config.gzip)
                .pool_max_idle_per_host(2)
                .build()
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            let handle = std::thread::Builder::new()
                .name(format!("http-sender-{}", id))
                .spawn(move || {
                    sender_loop(&rx, &client, &url, max_retries, &bytes_written, &requests_sent, &requests_failed);
                })
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            sender_threads.push(handle);
        }

        // Drop our copy of rx so it's only held by threads
        drop(rx);

        Ok(Self {
            config,
            buffer: Vec::with_capacity(1024 * 1024),
            tx: Some(tx),
            sender_threads,
            bytes_written,
            requests_sent,
            requests_failed,
        })
    }

    /// Prepare the buffered data as a ready-to-send payload and enqueue it.
    fn send_buffer(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let payload = match self.config.batch_format {
            HttpBatchFormat::Raw => std::mem::take(&mut self.buffer),
            HttpBatchFormat::Helios => {
                let mut events_data = std::mem::take(&mut self.buffer);
                if events_data.last() == Some(&b',') {
                    events_data.pop();
                }
                let mut payload = Vec::with_capacity(events_data.len() + 15);
                payload.extend_from_slice(b"{\"events\":[");
                payload.extend_from_slice(&events_data);
                payload.extend_from_slice(b"]}");
                payload
            }
        };

        if let Some(tx) = &self.tx {
            // Bounded channel provides backpressure — blocks if senders can't keep up
            tx.send(payload).map_err(|_| {
                Error::new(ErrorKind::BrokenPipe, "All sender threads have exited")
            })?;
        }

        self.buffer = Vec::with_capacity(self.config.batch_size);
        Ok(())
    }
}

impl OutputWriter for ConcurrentHttpWriter {
    fn write_batch(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(data);

        if self.buffer.len() >= self.config.batch_size {
            self.send_buffer()?;
        }

        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Send any remaining buffered data
        self.send_buffer()?;

        // Drop the sender to signal threads to drain and exit
        self.tx.take();

        // Wait for all sender threads to finish
        for handle in self.sender_threads.drain(..) {
            let _ = handle.join();
        }

        Ok(())
    }

    fn bytes_written(&self) -> u64 {
        self.bytes_written.load(Ordering::Relaxed)
    }
}

/// Sender thread main loop: receive payloads and POST them.
fn sender_loop(
    rx: &crossbeam::channel::Receiver<Vec<u8>>,
    client: &Client,
    url: &str,
    max_retries: u32,
    bytes_written: &AtomicU64,
    requests_sent: &AtomicU64,
    requests_failed: &AtomicU64,
) {
    while let Ok(payload) = rx.recv() {
        let data_len = payload.len() as u64;

        let mut last_error = None;
        let mut success = false;

        for attempt in 0..=max_retries {
            match client.post(url).body(payload.clone()).send() {
                Ok(response) => {
                    requests_sent.fetch_add(1, Ordering::Relaxed);
                    if response.status().is_success() {
                        bytes_written.fetch_add(data_len, Ordering::Relaxed);
                        success = true;
                        break;
                    } else {
                        last_error = Some(format!(
                            "HTTP {}: {}",
                            response.status(),
                            response.text().unwrap_or_default()
                        ));
                    }
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }

            if attempt < max_retries {
                std::thread::sleep(Duration::from_millis(100 << attempt));
            }
        }

        if !success {
            requests_failed.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "http-sender: dropped payload after {} retries: {}",
                max_retries,
                last_error.unwrap_or_default()
            );
        }
    }
}
