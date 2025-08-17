use aboss_task::{
    data_processor::DataProcessor,
    dto::{BinancePrice, GetPrice},
    models::MapData,
    routes,
    rpc_manager::RpcManager,
    utils::extract_symbol,
};
use actix_web::{App, HttpServer};
use std::{collections::HashMap, sync::Arc};
use tokio::spawn;
use tracing::info;

mod config;
use config::AppConfig;
/// Entry point for the `aboss_task` service.
///
/// # Overview
///
/// This main function does the following:
/// 1. Initializes logging using `tracing_subscriber`.
/// 2. Loads configuration from environment variables (`AppConfig`).
/// 3. Extracts symbols from the list of URLs to monitor.
/// 4. Initializes a `DataProcessor` per symbol for tracking streaming statistics.
/// 5. Spawns a `RpcManager` task for each URL to fetch data periodically.
/// 6. Starts an `actix_web` HTTP server exposing `/health` and `/stats` endpoints.
///
/// # Async Execution
///
/// Each `RpcManager` runs in its own async task (`tokio::spawn`) and continuously updates
/// the corresponding `DataProcessorWriter`. This ensures **single-writer, multi-reader**
/// safety, while the HTTP handlers read concurrently from `DataProcessorReader`.
///
/// # Shared State
///
/// All `DataProcessorReader`s are stored in a `HashMap<String, DataProcessorReader>`
/// wrapped in an `Arc` and exposed to `actix_web` using `web::Data<MapData>`. This
/// makes the statistics **read-only** and shareable across all web handlers.
///
/// # Server Bindings
///
/// The server binds to the IP and port provided in configuration (`AppConfig`).
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing logs
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Load configuration (URLs, interval, SMA size, IP, port)
    let config = AppConfig::from_env()?;

    // Log parsed configuration
    tracing::info!("Parsed URLs: {:?}", config.urls);
    tracing::info!("Interval: {:?}, SMA_N: {}", config.interval, config.sma_n);
    tracing::info!("IP: {}, PORT: {}", config.ip, config.port);

    // Initialize map of symbol -> DataProcessorReader
    let mut map = HashMap::new();
    let symbols: Vec<String> = config
        .urls
        .iter()
        .filter_map(|url| extract_symbol(url))
        .collect();

    // Spawn a `RpcManager` for each URL
    for (idx, url) in config.urls.into_iter().enumerate() {
        // Fetch initial data from remote endpoint
        let initial_data = RpcManager::<BinancePrice>::send_reqwest(&config.client, &url).await?;
        let client = config.client.clone();

        // Split a DataProcessor into a reader and writer
        let (reader, writer) = DataProcessor::split(config.sma_n, initial_data.get_price());

        // Insert reader into shared map
        map.insert(symbols[idx].clone(), reader);

        let interval = config.interval;
        // Spawn async task to continuously fetch and process prices
        spawn(async move {
            let rpc_manager = RpcManager::<BinancePrice>::new(interval, &url, client, writer);
            rpc_manager.init_run().await; // runs infinitely
        });
    }

    info!("STARTING SERVER");

    // Wrap the map in Arc and Data for actix-web shareable state
    let map_data = actix_web::web::Data::new(MapData {
        data: Arc::new(map),
    });

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(map_data.clone())
            .configure(routes::init)
    })
    .bind((config.ip, config.port))?
    .run()
    .await?;

    Ok(())
}
