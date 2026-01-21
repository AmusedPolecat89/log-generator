//! URL path generation.
//!
//! Pre-generates a pool of realistic URL paths.

/// Number of pre-generated paths.
pub const PATH_POOL_SIZE: usize = 4096;

/// Common path prefixes.
const PATH_PREFIXES: &[&str] = &[
    "/api/v1",
    "/api/v2",
    "/api",
    "/v1",
    "/v2",
    "/users",
    "/products",
    "/orders",
    "/auth",
    "/admin",
    "/static",
    "/assets",
    "/images",
    "/css",
    "/js",
    "/",
];

/// Common path segments.
const PATH_SEGMENTS: &[&str] = &[
    "list",
    "create",
    "update",
    "delete",
    "get",
    "search",
    "login",
    "logout",
    "register",
    "profile",
    "settings",
    "dashboard",
    "reports",
    "analytics",
    "health",
    "status",
    "metrics",
    "webhook",
    "callback",
    "download",
    "upload",
    "export",
    "import",
];

/// Common file extensions for static resources.
const EXTENSIONS: &[&str] = &[
    ".html",
    ".css",
    ".js",
    ".json",
    ".png",
    ".jpg",
    ".svg",
    ".ico",
    ".woff2",
    ".map",
];

/// Pre-generate a pool of URL paths.
pub fn generate_path_pool(rng: &mut fastrand::Rng) -> Box<[String; PATH_POOL_SIZE]> {
    let mut paths: Vec<String> = Vec::with_capacity(PATH_POOL_SIZE);

    for _ in 0..PATH_POOL_SIZE {
        paths.push(generate_random_path(rng));
    }

    paths.try_into().unwrap()
}

/// Generate a single random URL path.
fn generate_random_path(rng: &mut fastrand::Rng) -> String {
    let path_type = rng.u8(0..100);

    if path_type < 40 {
        // 40% - API paths with IDs
        let prefix = PATH_PREFIXES[rng.usize(0..PATH_PREFIXES.len())];
        let segment = PATH_SEGMENTS[rng.usize(0..PATH_SEGMENTS.len())];

        if rng.bool() {
            // With numeric ID
            format!("{}/{}/{}", prefix, segment, rng.u32(1..100000))
        } else {
            // Without ID
            format!("{}/{}", prefix, segment)
        }
    } else if path_type < 60 {
        // 20% - Static resources
        let hash: u32 = rng.u32(..);
        let ext = EXTENSIONS[rng.usize(0..EXTENSIONS.len())];
        format!("/static/{:08x}{}", hash, ext)
    } else if path_type < 75 {
        // 15% - User/resource paths with UUIDs
        let uuid = format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            rng.u32(..),
            rng.u16(..),
            rng.u16(..),
            rng.u16(..),
            rng.u64(..) & 0xFFFFFFFFFFFF
        );
        let resource = ["users", "products", "orders", "sessions"][rng.usize(0..4)];
        format!("/api/v1/{}/{}", resource, uuid)
    } else if path_type < 90 {
        // 15% - Query parameters
        let prefix = PATH_PREFIXES[rng.usize(0..8)]; // API prefixes only
        let segment = PATH_SEGMENTS[rng.usize(0..PATH_SEGMENTS.len())];
        let page = rng.u16(1..100);
        let limit = [10, 20, 25, 50, 100][rng.usize(0..5)];
        format!("{}/{}?page={}&limit={}", prefix, segment, page, limit)
    } else {
        // 10% - Root and simple paths
        let simple = [
            "/",
            "/index.html",
            "/favicon.ico",
            "/robots.txt",
            "/sitemap.xml",
            "/health",
            "/ready",
            "/live",
        ];
        simple[rng.usize(0..simple.len())].to_string()
    }
}

/// Fast path lookup from pre-generated pool.
#[inline(always)]
pub fn get_path(pool: &[String; PATH_POOL_SIZE], index: u16) -> &str {
    // Mask to PATH_POOL_SIZE - 1 (4095)
    let idx = (index as usize) & (PATH_POOL_SIZE - 1);
    unsafe { pool.get_unchecked(idx) }
}
