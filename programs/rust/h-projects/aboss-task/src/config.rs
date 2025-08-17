use dotenv::dotenv;
use reqwest::{Client, ClientBuilder};
use std::{env, time::Duration};

/// Default timeout for HTTP requests in milliseconds.
pub const DEFAULT_TIME_OUT: u64 = 1000;
/// Maximum number of idle connections per host for the HTTP client pool.
pub const POOL_MAX_IDLE_PER_HOST: usize = 1;
/// Default IP address to bind to if not provided in environment.
pub const DEFAULT_IP: &str = "127.0.0.1";
/// Default port to bind to if not provided in environment.
pub const DEFAULT_PORT: u16 = 8000;

/// Application configuration loaded from environment variables.
///
/// This struct holds all configuration needed for the application, including:
/// - Target URLs for fetching data
/// - Interval for polling
/// - SMA window size
/// - HTTP client instance
/// - IP and port for binding
pub struct AppConfig {
    /// List of URLs to fetch data from
    pub urls: Vec<String>,
    /// Polling interval
    pub interval: Duration,
    /// Window size for calculating SMA
    pub sma_n: usize,
    /// Reqwest HTTP client configured with timeout and connection pool
    pub client: Client,
    /// IP address for the service to bind to
    pub ip: String,
    /// Port for the service to bind to
    pub port: u16,
}

/// Helper function to clean URLs from extra characters like `[` and `]`.
fn clean_urls(url: &str) -> String {
    let url = url.trim_matches(|c| c == '[' || c == ']');
    url.to_string()
}

impl AppConfig {
    /// Load configuration from `.env` file and system environment variables.
    ///
    /// # Environment Variables
    /// - `URLS` (comma-separated list of URLs)
    /// - `INTERVAL` (polling interval in milliseconds)
    /// - `SMA_N` (SMA window size)
    /// - `TIME_OUT` (optional HTTP timeout in milliseconds)
    /// - `IP` (optional IP address to bind to)
    /// - `PORT` (optional port to bind to)
    ///
    /// # Returns
    /// Returns `Ok(AppConfig)` on success, or a boxed error if parsing fails.
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        // Load .env file if present
        dotenv().ok();

        // Read and clean URLs
        let urls: Vec<String> = std::env::var("URLS")?
            .split(',')
            .map(|s| clean_urls(s))
            .collect();

        // Parse interval and SMA window size
        let interval: u64 = env::var("INTERVAL")?.parse()?;
        let sma_n: usize = env::var("SMA_N")?.parse()?;

        // Parse optional timeout, fallback to default if missing or invalid
        let time_out = env::var("TIME_OUT")
            .map(|d| d.parse::<u64>().unwrap_or(DEFAULT_TIME_OUT))
            .unwrap_or(DEFAULT_TIME_OUT);
        let timeout = Duration::from_millis(time_out);

        // Optional IP and port, fallback to defaults
        let ip = env::var("IP").unwrap_or(DEFAULT_IP.to_string());
        let port = env::var("PORT")
            .map(|d| d.parse::<u16>().unwrap_or(DEFAULT_PORT))
            .unwrap_or(DEFAULT_PORT);

        // Build reqwest HTTP client with timeout and connection pool settings
        let client = ClientBuilder::new()
            .connect_timeout(timeout)
            .pool_max_idle_per_host(POOL_MAX_IDLE_PER_HOST)
            .pool_idle_timeout(timeout)
            .build()
            .expect("Error while building Client");

        Ok(Self {
            urls,
            interval: Duration::from_millis(interval),
            sma_n,
            client,
            ip,
            port,
        })
    }
}
