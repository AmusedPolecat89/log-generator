//! IP address generation.
//!
//! Pre-generates a pool of realistic IPv4 addresses for fast lookup.

use std::fmt::Write;

/// Number of pre-generated IP addresses (64K for full u16 range).
pub const IP_POOL_SIZE: usize = 65536;

/// Pre-generate a pool of IPv4 addresses.
///
/// Uses realistic distributions:
/// - Private ranges (10.x, 192.168.x, 172.16-31.x)
/// - Common public ranges
/// - Avoids reserved addresses
pub fn generate_ip_pool(rng: &mut fastrand::Rng) -> Box<[String; IP_POOL_SIZE]> {
    let mut ips: Vec<String> = Vec::with_capacity(IP_POOL_SIZE);

    for _ in 0..IP_POOL_SIZE {
        let ip = generate_random_ip(rng);
        ips.push(ip);
    }

    ips.try_into().unwrap()
}

/// Generate a single random IP address.
#[inline]
fn generate_random_ip(rng: &mut fastrand::Rng) -> String {
    let mut buf = String::with_capacity(15); // "xxx.xxx.xxx.xxx"

    // Choose IP type based on weighted distribution
    let ip_type = rng.u8(0..100);

    if ip_type < 30 {
        // 30% - Private 10.x.x.x
        write!(
            buf,
            "10.{}.{}.{}",
            rng.u8(0..=255),
            rng.u8(0..=255),
            rng.u8(1..=254)
        )
        .unwrap();
    } else if ip_type < 50 {
        // 20% - Private 192.168.x.x
        write!(
            buf,
            "192.168.{}.{}",
            rng.u8(0..=255),
            rng.u8(1..=254)
        )
        .unwrap();
    } else if ip_type < 60 {
        // 10% - Private 172.16-31.x.x
        write!(
            buf,
            "172.{}.{}.{}",
            rng.u8(16..=31),
            rng.u8(0..=255),
            rng.u8(1..=254)
        )
        .unwrap();
    } else {
        // 40% - Public IPs (avoiding reserved ranges)
        let first = loop {
            let f = rng.u8(1..=223);
            // Skip localhost, multicast, reserved
            if f != 10 && f != 127 && f != 0 {
                break f;
            }
        };
        write!(
            buf,
            "{}.{}.{}.{}",
            first,
            rng.u8(0..=255),
            rng.u8(0..=255),
            rng.u8(1..=254)
        )
        .unwrap();
    }

    buf
}

/// Fast IP lookup from pre-generated pool.
#[inline(always)]
pub fn get_ip(pool: &[String; IP_POOL_SIZE], index: u16) -> &str {
    // Safety: index is u16, pool size is 65536, so always in bounds
    unsafe { pool.get_unchecked(index as usize) }
}
