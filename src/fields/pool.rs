//! Pre-generated field pools for zero-allocation log generation.
//!
//! Pools are generated at startup and accessed via fast indexed lookups.

use super::ip::{generate_ip_pool, IP_POOL_SIZE};
use super::path::{generate_path_pool, PATH_POOL_SIZE};
use super::status::{StatusCodeStrings, StatusCodeTable};
use super::user_agent::{generate_ua_pool, UA_POOL_SIZE};

/// HTTP methods with their weights.
pub const METHODS: &[(&str, u8)] = &[
    ("GET", 60),
    ("POST", 20),
    ("PUT", 8),
    ("DELETE", 5),
    ("PATCH", 4),
    ("HEAD", 2),
    ("OPTIONS", 1),
];

/// HTTP protocols.
pub const PROTOCOLS: &[&str] = &["HTTP/1.0", "HTTP/1.1", "HTTP/2.0"];

/// Common referrers.
pub const REFERRERS: &[&str] = &[
    "-",
    "https://www.google.com/",
    "https://www.bing.com/",
    "https://www.facebook.com/",
    "https://twitter.com/",
    "https://www.reddit.com/",
    "https://www.linkedin.com/",
    "https://github.com/",
];

/// Log levels for syslog/JSON.
pub const LOG_LEVELS: &[(&str, u8)] = &[
    ("DEBUG", 5),
    ("INFO", 70),
    ("WARN", 15),
    ("ERROR", 8),
    ("FATAL", 2),
];

/// Service names for JSON logs.
pub const SERVICES: &[&str] = &[
    "api-gateway",
    "user-service",
    "auth-service",
    "payment-service",
    "order-service",
    "inventory-service",
    "notification-service",
    "search-service",
];

/// Hostnames for syslog.
pub const HOSTNAMES: &[&str] = &[
    "web-01",
    "web-02",
    "web-03",
    "api-01",
    "api-02",
    "worker-01",
    "worker-02",
    "db-01",
];

/// Container for all pre-generated field pools.
pub struct FieldPool {
    /// Pre-generated IP addresses (65536 entries)
    pub ips: Box<[String; IP_POOL_SIZE]>,
    /// Pre-generated URL paths (4096 entries)
    pub paths: Box<[String; PATH_POOL_SIZE]>,
    /// Pre-generated user agents (256 entries)
    pub user_agents: Box<[&'static str; UA_POOL_SIZE]>,
    /// HTTP method lookup table (256 entries, weighted)
    pub methods: [&'static str; 256],
    /// HTTP protocol lookup table
    pub protocols: [&'static str; 4],
    /// Referrer lookup table (256 entries)
    pub referrers: [&'static str; 256],
    /// Log level lookup table (256 entries, weighted)
    pub log_levels: [&'static str; 256],
    /// Service name lookup table
    pub services: [&'static str; 8],
    /// Hostname lookup table
    pub hostnames: [&'static str; 8],
    /// Status code tables
    pub status_codes: StatusCodeTable,
    /// Status code strings
    pub status_strings: StatusCodeStrings,
    /// Pre-generated usernames
    pub usernames: [&'static str; 64],
    /// Response size ranges (min, max) for different status codes
    pub response_sizes: ResponseSizeTable,
}

impl FieldPool {
    /// Create a new field pool with pre-generated values.
    pub fn new() -> Self {
        let mut rng = fastrand::Rng::new();

        Self {
            ips: generate_ip_pool(&mut rng),
            paths: generate_path_pool(&mut rng),
            user_agents: generate_ua_pool(&mut rng),
            methods: build_method_table(),
            protocols: ["HTTP/1.0", "HTTP/1.1", "HTTP/1.1", "HTTP/2.0"],
            referrers: build_referrer_table(),
            log_levels: build_log_level_table(),
            services: [
                "api-gateway",
                "user-service",
                "auth-service",
                "payment-service",
                "order-service",
                "inventory-service",
                "notification-service",
                "search-service",
            ],
            hostnames: [
                "web-01", "web-02", "web-03", "api-01", "api-02", "worker-01", "worker-02", "db-01",
            ],
            status_codes: StatusCodeTable::new(),
            status_strings: StatusCodeStrings::new(),
            usernames: build_username_table(),
            response_sizes: ResponseSizeTable::new(),
        }
    }

    /// Get an IP address by index.
    #[inline(always)]
    pub fn get_ip(&self, index: u16) -> &str {
        unsafe { self.ips.get_unchecked(index as usize) }
    }

    /// Get a path by index.
    #[inline(always)]
    pub fn get_path(&self, index: u16) -> &str {
        let idx = (index as usize) & (PATH_POOL_SIZE - 1);
        unsafe { self.paths.get_unchecked(idx) }
    }

    /// Get a user agent by index.
    #[inline(always)]
    pub fn get_user_agent(&self, index: u8) -> &'static str {
        unsafe { *self.user_agents.get_unchecked(index as usize) }
    }

    /// Get an HTTP method by index.
    #[inline(always)]
    pub fn get_method(&self, index: u8) -> &'static str {
        unsafe { *self.methods.get_unchecked(index as usize) }
    }

    /// Get an HTTP protocol by index.
    #[inline(always)]
    pub fn get_protocol(&self, index: u8) -> &'static str {
        unsafe { *self.protocols.get_unchecked((index & 3) as usize) }
    }

    /// Get a referrer by index.
    #[inline(always)]
    pub fn get_referrer(&self, index: u8) -> &'static str {
        unsafe { *self.referrers.get_unchecked(index as usize) }
    }

    /// Get a log level by index.
    #[inline(always)]
    pub fn get_log_level(&self, index: u8) -> &'static str {
        unsafe { *self.log_levels.get_unchecked(index as usize) }
    }

    /// Get a service name by index.
    #[inline(always)]
    pub fn get_service(&self, index: u8) -> &'static str {
        unsafe { *self.services.get_unchecked((index & 7) as usize) }
    }

    /// Get a hostname by index.
    #[inline(always)]
    pub fn get_hostname(&self, index: u8) -> &'static str {
        unsafe { *self.hostnames.get_unchecked((index & 7) as usize) }
    }

    /// Get a host by index (alias for get_hostname).
    #[inline(always)]
    pub fn get_host(&self, index: u8) -> &'static str {
        self.get_hostname(index)
    }

    /// Get a username by index.
    #[inline(always)]
    pub fn get_username(&self, index: u8) -> &'static str {
        unsafe { *self.usernames.get_unchecked((index & 63) as usize) }
    }
}

impl Default for FieldPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Response size ranges for realistic body_bytes values.
pub struct ResponseSizeTable {
    /// Size ranges: (min, max) for different content types
    ranges: [(u32, u32); 8],
}

impl ResponseSizeTable {
    pub fn new() -> Self {
        Self {
            ranges: [
                (0, 0),           // 204 No Content
                (50, 500),        // Small JSON responses
                (500, 5000),      // Medium responses
                (5000, 50000),    // Large responses
                (100, 1000),      // Error responses
                (1000, 100000),   // HTML pages
                (10000, 500000),  // Images/assets
                (100000, 5000000), // Large files
            ],
        }
    }

    /// Get a realistic response size for the given status code.
    #[inline(always)]
    pub fn get_size(&self, rng: &mut fastrand::Rng, status: u16) -> u32 {
        let range_idx = match status {
            204 => 0,
            200 | 201 => {
                // Vary based on another random value
                let r = rng.u8(..);
                if r < 100 {
                    1 // Small JSON
                } else if r < 200 {
                    2 // Medium
                } else {
                    3 // Large
                }
            }
            301 | 302 | 304 | 307 => 1, // Redirects - small
            400..=499 => 4,             // Client errors
            500..=599 => 4,             // Server errors
            _ => 2,                     // Default medium
        };

        let (min, max) = unsafe { *self.ranges.get_unchecked(range_idx) };
        if min == max {
            min
        } else {
            rng.u32(min..=max)
        }
    }
}

impl Default for ResponseSizeTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Build weighted method lookup table.
fn build_method_table() -> [&'static str; 256] {
    let mut table = ["GET"; 256];
    let mut idx = 0;

    for (method, weight) in METHODS {
        let count = (*weight as usize) * 256 / 100;
        for _ in 0..count {
            if idx < 256 {
                table[idx] = method;
                idx += 1;
            }
        }
    }

    // Fill remaining with GET
    while idx < 256 {
        table[idx] = "GET";
        idx += 1;
    }

    table
}

/// Build referrer lookup table.
fn build_referrer_table() -> [&'static str; 256] {
    let mut table = ["-"; 256];

    // 60% no referrer, 40% with referrer
    for i in 0..256 {
        if i >= 154 {
            // 40% with referrer
            let ref_idx = (i - 154) % (REFERRERS.len() - 1) + 1;
            table[i] = REFERRERS[ref_idx];
        }
    }

    table
}

/// Build log level lookup table.
fn build_log_level_table() -> [&'static str; 256] {
    let mut table = ["INFO"; 256];
    let mut idx = 0;

    for (level, weight) in LOG_LEVELS {
        let count = (*weight as usize) * 256 / 100;
        for _ in 0..count {
            if idx < 256 {
                table[idx] = level;
                idx += 1;
            }
        }
    }

    while idx < 256 {
        table[idx] = "INFO";
        idx += 1;
    }

    table
}

/// Build username lookup table.
fn build_username_table() -> [&'static str; 64] {
    [
        "-", "-", "-", "-", "-", "-", "-", "-", // 8x "-" for ~12.5% anonymous
        "-", "-", "-", "-", "-", "-", "-", "-", // Another 8
        "-", "-", "-", "-", "-", "-", "-", "-", // Another 8
        "-", "-", "-", "-", "-", "-", "-", "-", // Another 8 = 50% anonymous
        "john", "jane", "bob", "alice", "admin", "user", "guest", "test",
        "mike", "sarah", "david", "emma", "chris", "lisa", "tom", "anna",
        "api_user", "service", "system", "root", "www-data", "nginx", "app", "worker",
        "user1", "user2", "user3", "user4", "user5", "demo", "dev", "prod",
    ]
}
