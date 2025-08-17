# Aboss Task Crate

A real-time price tracking and statistics API for multiple trading symbols. It polls external APIs, calculates statistics including streaming mean and SMA, and serves the data via HTTP endpoints using Actix Web.

---

## How to Run the Program

1. **Clone the repository**

```bash
git clone <repo-url>
cd <repo-directory>
```

2. **Create a `.env` file** with configuration:

```env
URLS = ["https://api.binance.com/api/v3/ticker/price?symbol=ETHUSDT","https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT","https://api.binance.com/api/v3/ticker/price?symbol=SOLUSDT"]
INTERVAL=1000
SMA_N=4
TIME_OUT=1000
IP=127.0.0.1
PORT=8000
```

* `URLS`: Comma-separated list of API endpoints for each symbol.
* `INTERVAL`: Polling interval in milliseconds.
* `SMA_N`: Size of the Simple Moving Average (SMA) window.
* `TIME_OUT`: Reqwest client timeout in milliseconds.
* `IP` and `PORT`: Server bind address.

3. **Run the server**

```bash
cargo run
```

The server will start and continuously poll each API URL, updating statistics in real time.

---

## Example API Responses

### Health Check

**Request**

```http
GET /health
```

**Response**

```json
{
  "status": "ok"
}
```

---

### Single Symbol Stats

**Request**

```http
GET /stats?symbol=BTCUSDT
```

**Response**

```json
{
  "min": 117402.38,
  "max": 117463.86,
  "curr_avg": 117435.56191489362,
  "sma": 117454.33400000003,
  "data_point": 47
}
```

---

### All Symbols Stats

**Request**

```http
GET /stats/
```

**Response**

```json
[
  {
    "SOLUSDT": {
      "min": 188.32,
      "max": 188.62,
      "curr_avg": 188.43023255813955,
      "sma": 188.60399999999998,
      "data_point": 43
    }
  },
  {
    "ETHUSDT": {
      "min": 4413.98,
      "max": 4423.18,
      "curr_avg": 4418.201162790698,
      "sma": 4422.064000000002,
      "data_point": 43
    }
  },
  {
    "BTCUSDT": {
      "min": 117402.38,
      "max": 117463.86,
      "curr_avg": 117433.81558139535,
      "sma": 117457.36200000002,
      "data_point": 43
    }
  }
]
```

---

This documentation provides enough information to run the server and understand the JSON structure returned by each endpoint.
