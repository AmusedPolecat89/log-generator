//! User agent string generation.
//!
//! Provides realistic user agent strings for various browsers and devices.

/// Number of pre-generated user agents.
pub const UA_POOL_SIZE: usize = 256;

/// Realistic user agent strings.
const USER_AGENTS: &[&str] = &[
    // Chrome - Desktop
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    // Firefox - Desktop
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (X11; Linux x86_64; rv:121.0) Gecko/20100101 Firefox/121.0",
    // Safari - Desktop
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15",
    // Edge
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0",
    // Mobile - iOS
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (iPad; CPU OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Mobile/15E148 Safari/604.1",
    // Mobile - Android
    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36",
    "Mozilla/5.0 (Linux; Android 14; SM-S918B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36",
    // Bots
    "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
    "Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)",
    "Mozilla/5.0 (compatible; YandexBot/3.0; +http://yandex.com/bots)",
    // API clients
    "curl/8.4.0",
    "python-requests/2.31.0",
    "Go-http-client/2.0",
    "axios/1.6.2",
    "PostmanRuntime/7.35.0",
    "okhttp/4.12.0",
    // CLI tools
    "Wget/1.21.4",
    "HTTPie/3.2.2",
];

/// Pre-generate a pool of user agent strings by repeating and shuffling.
pub fn generate_ua_pool(rng: &mut fastrand::Rng) -> Box<[&'static str; UA_POOL_SIZE]> {
    let mut uas: Vec<&'static str> = Vec::with_capacity(UA_POOL_SIZE);

    // Fill pool by repeating base list with weighted distribution
    // Chrome gets higher weight as it's most common
    for _ in 0..UA_POOL_SIZE {
        let ua_type = rng.u8(0..100);
        let ua = if ua_type < 50 {
            // 50% Chrome
            USER_AGENTS[rng.usize(0..4)]
        } else if ua_type < 65 {
            // 15% Firefox
            USER_AGENTS[rng.usize(4..7)]
        } else if ua_type < 75 {
            // 10% Safari/Edge
            USER_AGENTS[rng.usize(7..9)]
        } else if ua_type < 90 {
            // 15% Mobile
            USER_AGENTS[rng.usize(9..13)]
        } else if ua_type < 95 {
            // 5% Bots
            USER_AGENTS[rng.usize(13..16)]
        } else {
            // 5% API clients/CLI
            USER_AGENTS[rng.usize(16..USER_AGENTS.len())]
        };
        uas.push(ua);
    }

    uas.try_into().unwrap()
}

/// Fast user agent lookup from pre-generated pool.
#[inline(always)]
pub fn get_user_agent(pool: &[&'static str; UA_POOL_SIZE], index: u8) -> &'static str {
    unsafe { *pool.get_unchecked(index as usize) }
}
