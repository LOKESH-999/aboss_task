use std::mem::{align_of, size_of, transmute};

use serde::{ser::SerializeMap, Deserialize, Serialize, Serializer};

use crate::data_processor::RawData;

/// Response for health check endpoints
///
/// Used to return a simple JSON status indicating that the service is healthy.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Always "ok" when the service is healthy
    pub status: &'static str,
}

impl HealthResponse {
    /// Returns a JSON-formatted health status string.
    ///
    /// # Example
    ///
    /// ```
    /// use aboss_task::dto::HealthResponse;
    /// let json_str = HealthResponse::health_status_json_string_default();
    /// assert_eq!(json_str, r#"{"status":"ok"}"#);
    /// ```
    pub fn health_status_json_string_default() -> &'static str {
        r#"{"status":"ok"}"#
    }
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self { status: "ok" }
    }
}

/// Binance API response structure for a trading pair's price.
///
/// # Fields
/// - `symbol`: The trading pair symbol, e.g., `"BTCUSDT"`.
/// - `price`: Current price, parsed from a string in the JSON response.
#[derive(Debug, Serialize, Deserialize)]
pub struct BinancePrice {
    pub symbol: String,
    #[serde(deserialize_with = "de_str_to_f64")]
    pub price: f64,
}

/// Deserialize a string to `f64` for Binance API responses
///
/// # Safety
/// Assumes the API sends a valid numeric string.
/// Returns a serde error if parsing fails.
fn de_str_to_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<f64>().map_err(serde::de::Error::custom)
}

/// Trait to generalize extracting a price from different response types
pub trait GetPrice {
    /// Returns the current price as `f64`.
    fn get_price(&self) -> f64;
}

impl GetPrice for BinancePrice {
    fn get_price(&self) -> f64 {
        self.price
    }
}

/// Response struct for statistical data.
///
/// Mirrors `RawData` exactly, so that it can be safely transmuted.
/// Fields:
/// - `min`: minimum value observed
/// - `max`: maximum value observed
/// - `curr_avg`: streaming mean of all observed values
/// - `sma`: current Simple Moving Average
/// - `data_point`: number of data points processed
#[repr(C)]
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub min: f64,
    pub max: f64,
    pub curr_avg: f64,
    pub sma: f64,
    pub data_point: u64,
}

impl From<RawData> for StatsResponse {
    fn from(value: RawData) -> Self {
        // SAFETY: RawData and StatsResponse have identical memory layout
        unsafe { transmute::<RawData, StatsResponse>(value) }
    }
}

/// Combined response for all state statistics of a symbol.
///
/// Used for serializing symbol -> stats mapping.
pub struct AllStatesResponse {
    pub symbol: String,
    pub stats: StatsResponse,
}

impl Serialize for AllStatesResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as a JSON map { "symbol": { stats } }
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.symbol, &self.stats)?;
        map.end()
    }
}

// Compile-time checks to ensure transmute safety between RawData and StatsResponse
const _: () = assert!(size_of::<RawData>() == size_of::<StatsResponse>());
const _: () = assert!(align_of::<RawData>() == align_of::<StatsResponse>());

// Future: Add memory offset checks per field in unit tests
