//! HTTP status code generation.
//!
//! Provides realistic distributions of HTTP status codes.

/// HTTP status codes with their weights for normal operation.
/// Format: (status_code, weight)
const SUCCESS_CODES: &[(u16, u8)] = &[
    (200, 70), // OK - most common
    (201, 5),  // Created
    (204, 5),  // No Content
    (206, 2),  // Partial Content
    (301, 3),  // Moved Permanently
    (302, 3),  // Found
    (304, 10), // Not Modified - caching
    (307, 2),  // Temporary Redirect
];

/// Client error codes (4xx).
const CLIENT_ERROR_CODES: &[(u16, u8)] = &[
    (400, 20), // Bad Request
    (401, 25), // Unauthorized
    (403, 15), // Forbidden
    (404, 30), // Not Found - most common error
    (405, 3),  // Method Not Allowed
    (408, 2),  // Request Timeout
    (409, 2),  // Conflict
    (422, 2),  // Unprocessable Entity
    (429, 1),  // Too Many Requests
];

/// Server error codes (5xx).
const SERVER_ERROR_CODES: &[(u16, u8)] = &[
    (500, 50), // Internal Server Error
    (502, 20), // Bad Gateway
    (503, 20), // Service Unavailable
    (504, 10), // Gateway Timeout
];

/// Pre-computed lookup tables for fast status code selection.
pub struct StatusCodeTable {
    /// Success codes weighted table (256 entries)
    success: [u16; 256],
    /// Client error codes weighted table (256 entries)
    client_error: [u16; 256],
    /// Server error codes weighted table (256 entries)
    server_error: [u16; 256],
}

impl StatusCodeTable {
    /// Create a new status code lookup table.
    pub fn new() -> Self {
        Self {
            success: build_weighted_table(SUCCESS_CODES),
            client_error: build_weighted_table(CLIENT_ERROR_CODES),
            server_error: build_weighted_table(SERVER_ERROR_CODES),
        }
    }

    /// Get a success status code (2xx, 3xx).
    #[inline(always)]
    pub fn success(&self, index: u8) -> u16 {
        unsafe { *self.success.get_unchecked(index as usize) }
    }

    /// Get a client error status code (4xx).
    #[inline(always)]
    pub fn client_error(&self, index: u8) -> u16 {
        unsafe { *self.client_error.get_unchecked(index as usize) }
    }

    /// Get a server error status code (5xx).
    #[inline(always)]
    pub fn server_error(&self, index: u8) -> u16 {
        unsafe { *self.server_error.get_unchecked(index as usize) }
    }

    /// Get a status code based on error rate.
    /// `error_rate` is 0.0-1.0 (0-100%)
    #[inline(always)]
    pub fn get_status(&self, rng: &mut fastrand::Rng, error_rate: f32) -> u16 {
        let roll = rng.f32();

        if roll < error_rate {
            // Error case
            if rng.bool() {
                // 50% client error
                self.client_error(rng.u8(..))
            } else {
                // 50% server error
                self.server_error(rng.u8(..))
            }
        } else {
            // Success case
            self.success(rng.u8(..))
        }
    }
}

impl Default for StatusCodeTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a 256-entry weighted lookup table from code/weight pairs.
fn build_weighted_table(codes: &[(u16, u8)]) -> [u16; 256] {
    let mut table = [200u16; 256];

    let total_weight: u32 = codes.iter().map(|(_, w)| *w as u32).sum();

    let mut idx = 0;
    for (code, weight) in codes {
        // Calculate how many entries this code gets (proportional to weight)
        let entries = ((*weight as u32) * 256 / total_weight) as usize;
        for _ in 0..entries {
            if idx < 256 {
                table[idx] = *code;
                idx += 1;
            }
        }
    }

    // Fill remaining entries with the first code
    while idx < 256 {
        table[idx] = codes[0].0;
        idx += 1;
    }

    table
}

/// Pre-computed status code strings for even faster output.
pub struct StatusCodeStrings {
    strings: [&'static str; 600],
}

impl StatusCodeStrings {
    pub fn new() -> Self {
        let mut strings = [""; 600];

        // Common status codes
        strings[200] = "200";
        strings[201] = "201";
        strings[204] = "204";
        strings[206] = "206";
        strings[301] = "301";
        strings[302] = "302";
        strings[304] = "304";
        strings[307] = "307";
        strings[400] = "400";
        strings[401] = "401";
        strings[403] = "403";
        strings[404] = "404";
        strings[405] = "405";
        strings[408] = "408";
        strings[409] = "409";
        strings[422] = "422";
        strings[429] = "429";
        strings[500] = "500";
        strings[502] = "502";
        strings[503] = "503";
        strings[504] = "504";

        Self { strings }
    }

    /// Get status code as a static string.
    #[inline(always)]
    pub fn get(&self, code: u16) -> &'static str {
        if (code as usize) < self.strings.len() {
            let s = unsafe { *self.strings.get_unchecked(code as usize) };
            if !s.is_empty() {
                return s;
            }
        }
        // Fallback - shouldn't happen with our codes
        match code {
            200 => "200",
            404 => "404",
            500 => "500",
            _ => "200",
        }
    }
}

impl Default for StatusCodeStrings {
    fn default() -> Self {
        Self::new()
    }
}
