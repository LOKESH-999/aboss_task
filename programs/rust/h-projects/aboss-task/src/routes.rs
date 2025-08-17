use actix_web::{
    HttpResponse, HttpResponseBuilder, get,
    http::StatusCode,
    web::{Data, Query, ServiceConfig},
};

use crate::{
    dto::{AllStatesResponse, HealthResponse, StatsResponse},
    models::{MapData, QuerryData},
};

/// Health check endpoint.
///
/// Returns a simple JSON string indicating the service status.
/// Example response:
/// ```json
/// { "status": "ok" }
/// ```
#[get("/health")]
async fn health() -> HttpResponse {
    HttpResponseBuilder::new(StatusCode::OK)
        .body(HealthResponse::health_status_json_string_default())
}

/// Get statistics for a specific symbol.
///
/// - `querry`: Query parameter containing the `symbol` to look up.
/// - `map`: Shared read-only reference to `MapData` containing all symbol readers.
///
/// Returns HTTP 200 with JSON body containing the stats for the symbol if it exists,
/// or HTTP 204 if the symbol is not found.
///
/// Example JSON response:
/// ```json
/// {
///   "min": 123.45,
///   "max": 234.56,
///   "curr_avg": 200.12,
///   "sma": 210.34,
///   "data_point": 50
/// }
/// ```
#[get("/stats")]
async fn stat(querry: Query<QuerryData>, map: Data<MapData>) -> HttpResponse {
    let res = map.data.get(&querry.symbol);

    if let Some(pair_data) = res {
        let data: StatsResponse = pair_data.read().into();
        HttpResponseBuilder::new(StatusCode::OK)
            .body(serde_json::to_string(&data).unwrap_or("Error while Sending".to_string()))
    } else {
        HttpResponseBuilder::new(StatusCode::NO_CONTENT)
            .body("The content you search does not exist")
    }
}

/// Get statistics for all symbols.
///
/// - `map`: Shared read-only reference to `MapData`.
///
/// Returns HTTP 200 with JSON array containing statistics for all available symbols.
#[get("/stats/")]
async fn stats(map: Data<MapData>) -> HttpResponse {
    let mut result = Vec::with_capacity(map.data.len() + 1);

    for (symbol,reader) in map.data.iter() {
        let data: StatsResponse = reader.read().into();
        let val = AllStatesResponse{symbol:symbol.clone(),stats:data};
        result.push(val);
    }

    HttpResponseBuilder::new(StatusCode::OK)
        .body(serde_json::to_string(&result).unwrap_or("Error while Sending".to_string()))
}

/// Initialize all routes for the application.
///
/// Registers the health and stats endpoints with the Actix-web service configuration.
pub fn init(cfg: &mut ServiceConfig) {
    cfg.service(health).service(stat).service(stats);
}
