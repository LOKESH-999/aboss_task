use crate::data_processor::DataProcessorReader;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};

/// A shared, thread-safe, **read-only** mapping from string keys (symbols) to data processors.
///
/// This map is intended to be **read-only**; modifications are not supported after creation.
/// It can be safely shared between multiple threads and is suitable for use with
/// `actix_web::web::Data` for storing symbolstats reader.
#[derive(Clone)]
pub struct MapData {
    /// The underlying map wrapped in an `Arc` for shared ownership across threads.
    pub data: Arc<HashMap<String, DataProcessorReader>>,
}

/// Structure representing a query request for a specific symbol.
/// Typically deserialized from JSON input.
#[derive(Debug, Deserialize)]
pub struct QuerryData {
    /// The symbol to query (e.g., "BTCUSDT").
    pub symbol: String,
}

// Safety: `MapData` can be safely sent and shared across threads because
// `Arc<HashMap<...>>` is inherently thread-safe for read-only access.
unsafe impl Send for MapData {}
unsafe impl Sync for MapData {}
