# stats Specification

## Purpose

Provides aggregated in-memory statistics for the dashboard: top rules, countries, methods, paths, FQDNs, time-series evolution, and request totals. Stats are collected by the WAF and Jail pipelines and served via REST endpoints.

## Architecture

### StatsCollector (in-memory)

- **Atomic counters**: `total_allowed`, `total_blocked` — lock-free via `AtomicU64`
- **Top N maps**: `top_rules` (`HashMap<i32, u64>`), `top_countries`, `top_methods`, `top_paths`, `top_fqdns` — protected by `Mutex`
- **Time series**: `minute_series` (60 buckets), `hour_series` (24 buckets), `day_series` (31 buckets) — circular buffers
- **Method series**: `method_series` (`HashMap<String, Vec<MethodBucket>>`) — per-HTTP-method time series
- **Persistence**: Snapshot to `stats_cache` table every 30 minutes; loaded on startup

### API Endpoints

All endpoints are mounted under `/api/v1/stats/` and return `ApiResponse` JSON.

| Method | Path | Handler | Description |
|---|---|---|---|
| GET | `/api/v1/stats/top_rules` | `read_top_rules` | Top 10 rules by blocked count, with names |
| GET | `/api/v1/stats/top_countries` | `read_top_countries` | Top 10 countries by blocked count |
| GET | `/api/v1/stats/top_methods` | `read_top_methods` | Top 10 HTTP methods by blocked count |
| GET | `/api/v1/stats/top_paths` | `read_top_paths` | Top 10 paths by blocked count |
| GET | `/api/v1/stats/top_fqdns` | `read_top_fqdns` | Top 10 FQDNs by blocked count |
| GET | `/api/v1/stats/evolution` | `read_evolution` | Time-series evolution (blocked/allowed) |
| GET | `/api/v1/stats/evolution_by_method` | `read_evolution_by_method` | Time-series per HTTP method |
| GET | `/api/v1/stats/info` | `read_info_handler` | Aggregated totals (`total` or `filtered`) |

## Requirements

### Requirement: Top Rules returns rule names

The `read_top_rules` handler returns `Vec<(String, i32, f32)>` where the first element is the rule **name** (not the rule ID). For each rule ID from `StatsCollector::get_top_rules()`, look up the corresponding `CacheRule` in `AppState.rules`. If found, use `rule.name`; if not found, fall back to `"Unknown rule #{id}"`.

**Response format**: `{ "status": 200, "message": "Top rules", "data": [["Auth Guard", 3, 60.0], ["Path Scanner", 2, 40.0]] }`

#### Scenario: Happy path — all rules exist in cache
Given the stats collector has recorded hits for rules with IDs 1, 2, 3
And the rules cache contains rules with names "Auth Guard", "Path Scanner", "Geo Block"
When the user navigates to the Charts page
Then the Top Rules pie chart shows "Auth Guard", "Path Scanner", "Geo Block" as labels

#### Scenario: Rule deleted from DB but still in stats
Given the stats collector has recorded hits for rule ID 99
And no rule with ID 99 exists in the rules cache
When the user navigates to the Charts page
Then the Top Rules pie chart shows "Unknown rule #99" as the label

#### Scenario: Empty top rules
Given the stats collector has no recorded rule hits
When the user navigates to the Charts page
Then the Top Rules pie chart shows "No rule data available"

### Requirement: Top Countries returns country names

The `read_top_countries` handler returns `Vec<(String, i32, f32)>` where the first element is the country code/name.

**Response format**: `{ "status": 200, "message": "Top countries", "data": [["US", 150, 45.0], ["CN", 80, 24.0]] }`

#### Scenario: Happy path
Given the stats collector has recorded blocked requests from "US" (150) and "CN" (80)
When the user views the Top Countries chart
Then the chart shows "US" with 150 and "CN" with 80

#### Scenario: Empty top countries
Given the stats collector has no country data
When the user views the Top Countries chart
Then the chart shows "No country data available"

### Requirement: Top Methods returns HTTP methods

The `read_top_methods` handler returns `Vec<(String, i32, f32)>` where the first element is the HTTP method.

**Response format**: `{ "status": 200, "message": "Top methods", "data": [["POST", 200, 50.0], ["GET", 150, 37.5]] }`

#### Scenario: Happy path
Given the stats collector has recorded blocked requests for POST (200) and GET (150)
When the user views the Top Methods chart
Then the chart shows "POST" with 200 and "GET" with 150

### Requirement: Top Paths returns request paths

The `read_top_paths` handler returns `Vec<(String, i32, f32)>` where the first element is the URL path.

**Response format**: `{ "status": 200, "message": "Top paths", "data": [["/wp-admin", 50, 25.0], ["/xmlrpc.php", 30, 15.0]] }`

#### Scenario: Happy path
Given the stats collector has recorded blocked requests for "/wp-admin" (50) and "/xmlrpc.php" (30)
When the user views the Top Paths chart
Then the chart shows "/wp-admin" with 50 and "/xmlrpc.php" with 30

### Requirement: Top FQDNs returns fully qualified domain names

The `read_top_fqdns` handler returns `Vec<(String, i32, f32)>` where the first element is the FQDN.

**Response format**: `{ "status": 200, "message": "Top FQDNs", "data": [["example.com", 100, 50.0], ["test.org", 50, 25.0]] }`

#### Scenario: Happy path
Given the stats collector has recorded blocked requests for "example.com" (100) and "test.org" (50)
When the user views the Top FQDNs chart
Then the chart shows "example.com" with 100 and "test.org" with 50

### Requirement: Evolution returns time-series data

The `read_evolution` handler accepts query parameters `unit` (`day|hour|minute`) and `last` (number of periods). Returns blocked and allowed series.

**Response format**:
```json
{
  "status": 200,
  "message": "Request evolution",
  "data": [
    {"id": "blocked", "data": [{"x": "2026-09-22T10:00:00Z", "y": 5}, ...]},
    {"id": "allowed", "data": [{"x": "2026-09-22T10:00:00Z", "y": 10}, ...]}
  ]
}
```

#### Scenario: Default unit is "day"
Given no query parameters are provided
When the evolution endpoint is called
Then it returns day-series data (31 buckets max)

#### Scenario: Hour unit with last N
Given `unit=hour` and `last=6`
When the evolution endpoint is called
Then it returns the last 6 hourly buckets

#### Scenario: Empty evolution
Given no requests have been recorded
When the evolution endpoint is called
Then it returns empty arrays for both blocked and allowed

### Requirement: Evolution by Method returns per-method time series

The `read_evolution_by_method` handler accepts the same query parameters as evolution. Returns time-series data grouped by HTTP method.

**Response format**:
```json
{
  "status": 200,
  "message": "Evolution by method",
  "data": [
    {"id": "GET", "data": [{"x": "2026-09-22T10:00:00Z", "y": 3}, ...]},
    {"id": "POST", "data": [{"x": "2026-09-22T10:00:00Z", "y": 7}, ...]}
  ]
}
```

#### Scenario: Happy path
Given requests have been recorded for GET and POST methods
When the evolution_by_method endpoint is called
Then it returns per-method time series

### Requirement: Info returns aggregated totals

The `read_info_handler` handler accepts a query parameter `option` with values `total` or `filtered`.

- `total`: returns `total_allowed + total_blocked`
- `filtered`: returns `total_blocked`

#### Scenario: Total option
Given `option=total`
When the info endpoint is called
Then it returns the sum of allowed and blocked requests

#### Scenario: Filtered option
Given `option=filtered`
When the info endpoint is called
Then it returns only the blocked count

#### Scenario: Invalid option
Given `option=invalid`
When the info endpoint is called
Then it returns 400 BAD_REQUEST with message "Parameter option must be 'total' or 'filtered'"

#### Scenario: Missing option
Given no option parameter
When the info endpoint is called
Then it returns 400 BAD_REQUEST with message "Option parameter is required"

### Requirement: StatsCollector records blocked requests

The `record_blocked` method increments `total_blocked` and updates all top-N maps and time series.

#### Scenario: Record blocked with all fields
Given a blocked request with rule_id=1, country="US", method="POST", path="/admin", fqdn="example.com"
When `record_blocked` is called
Then total_blocked increments by 1
And top_rules[1] increments by 1
And top_countries["US"] increments by 1
And top_methods["POST"] increments by 1
And top_paths["/admin"] increments by 1
And top_fqdns["example.com"] increments by 1
And the time series buckets are updated

#### Scenario: Record blocked without rule_id
Given a blocked request with rule_id=None
When `record_blocked` is called
Then total_blocked increments by 1
But top_rules is NOT updated

### Requirement: StatsCollector records allowed requests

The `record_allowed` method increments `total_allowed` and updates method/path/FQDN maps and time series.

#### Scenario: Record allowed
Given an allowed request with method="GET", path="/health", fqdn="example.com"
When `record_allowed` is called
Then total_allowed increments by 1
And top_methods["GET"] increments by 1
And top_paths["/health"] increments by 1
And top_fqdns["example.com"] increments by 1
And the time series buckets are updated

### Requirement: Stats persistence

Stats are persisted as JSON to the `stats_cache` table every 30 minutes and loaded on startup.

#### Scenario: Persist snapshot
Given the StatsCollector has accumulated data
When the background persist task runs
Then a JSON snapshot is saved to the `stats_cache` table

#### Scenario: Load snapshot on startup
Given a previous snapshot exists in `stats_cache`
When the application starts
Then the StatsCollector is initialized from the snapshot
And all counters and maps are restored

#### Scenario: No snapshot on startup
Given no snapshot exists in `stats_cache`
When the application starts
Then the StatsCollector starts with empty counters