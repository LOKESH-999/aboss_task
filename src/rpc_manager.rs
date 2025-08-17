use std::{marker::PhantomData, time::Duration};

use reqwest::{Client, Error};
use serde::de::DeserializeOwned;
use tokio::time::interval;
use tracing::error;

use crate::{data_processor::DataProcessorWriter, dto::GetPrice};

/// A generic RPC manager that periodically fetches data from a given HTTP endpoint
/// and updates a `DataProcessorWriter` with the latest value.
///
/// # Type Parameters
/// - `T`: The response type from the endpoint, which must implement `DeserializeOwned`
///   (to allow JSON deserialization) and `GetPrice` (to extract the price from the response).
pub struct RpcManager<'a, T>
where
    T: DeserializeOwned + GetPrice,
{
    /// Interval between successive requests.
    interval: Duration,

    /// Full path to query, including query parameters.
    path: &'a str,

    /// Writer for updating shared streaming statistics.
    data_processor_writer: DataProcessorWriter,

    /// Reqwest client used for HTTP requests.
    client_manager: Client,

    /// Phantom data to tie the generic response type to this struct.
    _response_phantom_data: PhantomData<T>,
}

impl<'a, ResponseType> RpcManager<'a, ResponseType>
where
    ResponseType: DeserializeOwned + GetPrice,
{
    /// Constructs a new `RpcManager`.
    ///
    /// # Parameters
    /// - `interval`: Duration between HTTP requests.
    /// - `path`: URL path for the RPC endpoint.
    /// - `client_manager`: Reqwest client to perform requests.
    /// - `data_processor_writer`: Writer to update statistics with fetched prices.
    pub fn new(
        interval: Duration,
        path: &'a str,
        client_manager: Client,
        data_processor_writer: DataProcessorWriter,
    ) -> Self {
        Self {
            interval,
            path,
            data_processor_writer,
            client_manager,
            _response_phantom_data: PhantomData,
        }
    }

    /// Sends a single HTTP GET request to the given path and attempts to deserialize
    /// the response into `ResponseType`.
    ///
    /// # Parameters
    /// - `client_manager`: Reqwest client.
    /// - `path`: URL path for the RPC endpoint.
    ///
    /// # Returns
    /// `Result<ResponseType, Error>` containing either the deserialized response or an error.
    pub async fn send_reqwest(client_manager: &Client, path: &str) -> Result<ResponseType, Error> {
        let res = client_manager.get(path).send().await?;
        res.json::<ResponseType>().await
    }

    /// Continuously fetches data from the RPC endpoint at the configured interval.
    ///
    /// On successful fetch, it extracts the price using `GetPrice` and writes it to
    /// the `DataProcessorWriter`. Errors during fetching or deserialization are logged
    /// but do not stop the loop.
    ///
    /// # Note
    /// This function never returns (`-> !`) as it loops indefinitely.
    pub async fn init_run(self) -> ! {
        let client_manager = &self.client_manager;
        let path = self.path;
        let mut ticker = interval(self.interval);
        loop {
            let res = Self::send_reqwest(client_manager, path).await;
            match res {
                Ok(price_data) => {
                    // Extract price and update the shared data processor
                    let price = price_data.get_price();
                    self.data_processor_writer.write(price);
                }
                Err(e) => {
                    // Log errors and continue
                    error!("Error while fetching RPC data: [{:?}]", e);
                    continue;
                }
            }
            // Wait for the configured interval before the next request
            ticker.tick().await;
        }
    }
}

/// Safety: `RpcManager` can be sent between threads since all its members are `Send`.
unsafe impl<'a, T: DeserializeOwned + GetPrice> Send for RpcManager<'a, T> {}
