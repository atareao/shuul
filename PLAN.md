# Log Viewer Implementation Plan

> **Goal:** Add an in-memory log viewer to the Shuul frontend so the user can see real-time WAF/Jail events without SSH/terminal access.

**Architecture:** Ring buffer (VecDeque) in the backend with configurable capacity (1000/5000/10000/20000). A shared `audit_log!` macro pushes to both stdout (existing tracing) and the ring buffer. Frontend polls `GET /api/v1/logs` and renders a paginated table with expandable JSON detail rows and event-type filter buttons.

**Tech Stack:** Rust/Axum, React 19/Ant Design 6, TypeScript, no DB/persistence.

---

## Files to Modify

| File | Action | Responsibility |
|---|---|---|
| `backend/src/models/log_collector.rs` | Create | LogEntry, LogCollector, shared audit_log! macro, global push_log() |
| `backend/src/models/mod.rs` | Modify | Export log_collector, add LogCollector to AppState |
| `backend/src/http/log.rs` | Create | GET /api/v1/logs + PUT /api/v1/logs/capacity handlers |
| `backend/src/http/mod.rs` | Modify | Add mod log, pub use log_router |
| `backend/src/http/shuul.rs` | Modify | Remove local audit_log! macro, import from models |
| `backend/src/http/report.rs` | Modify | Remove local audit_log! macro, import from models |
| `backend/src/main.rs` | Modify | Init LogCollector, add /logs to protected_routes |
| `frontend/src/pages/admin/logs_page.tsx` | Create | LogsPage component |
| `frontend/src/layouts/admin_layout.tsx` | Modify | Add "Logs" menu item |
| `frontend/src/App.tsx` | Modify | Add /admin/logs route |

---

### Task 1: Backend — LogCollector + shared macro

**Files:**
- Create: `backend/src/models/log_collector.rs`
- Modify: `backend/src/models/mod.rs` (add `pub mod log_collector;`, add field to AppState)

**Interfaces:**
- Produces: `LogEntry` struct, `LogCollector`, `audit_log!` macro, `push_log()` fn, `LOG_COLLECTOR` global

- [ ] **Step 1: Create `backend/src/models/log_collector.rs`**

```rust
//! # Log Collector — In-memory ring buffer for frontend log viewer
//!
//! Provides a global ring buffer (`LOG_COLLECTOR`) that captures WAF/Jail events
//! emitted by the `audit_log!` macro. Capacity is configurable at runtime.
//!
//! The macro both logs to `tracing::info!` (stdout, existing behavior) and pushes
//! a `LogEntry` to the ring buffer for the frontend.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;

/// A single log entry captured from the WAF/Jail pipelines.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub ts: String,
    pub event: String,
    pub pipeline: String,
    pub ip: Option<String>,
    pub country: Option<String>,
    pub rule_id: Option<i32>,
    pub rule_name: Option<String>,
    pub path: Option<String>,
    pub method: Option<String>,
    pub query: Option<String>,
    pub ua: Option<String>,
    pub fqdn: Option<String>,
    pub referer: Option<String>,
    pub status_code: Option<i32>,
    pub profile: Option<String>,
    pub reason: Option<String>,
}

/// Ring buffer collector with configurable capacity.
pub struct LogCollector {
    buffer: VecDeque<LogEntry>,
    capacity: usize,
}

impl LogCollector {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, entry: LogEntry) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(entry);
    }

    /// Returns all entries (front-to-back, oldest first).
    pub fn all(&self) -> Vec<LogEntry> {
        self.buffer.iter().cloned().collect()
    }

    pub fn set_capacity(&mut self, new_cap: usize) {
        self.capacity = new_cap;
        while self.buffer.len() > self.capacity {
            self.buffer.pop_front();
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}

// ── Global singleton ──

use std::sync::LazyLock;

pub static LOG_COLLECTOR: LazyLock<Mutex<LogCollector>> =
    LazyLock::new(|| Mutex::new(LogCollector::new(1000)));

/// Push a log entry into the global ring buffer (fire-and-forget).
pub fn push_log(entry: LogEntry) {
    if let Ok(mut collector) = LOG_COLLECTOR.lock() {
        collector.push(entry);
    }
}

/// Build a LogEntry from raw fields (used by the macro).
#[macro_export]
macro_rules! audit_log {
    ($category:expr, $($key:ident : $value:expr),* $(,)?) => {{
        // 1. Build JSON for tracing (existing behavior)
        let json_value = serde_json::json!({
            "event": $category,
            "ts": chrono::Utc::now().to_rfc3339(),
            $($key: $value,)*
        });
        tracing::info!("[{}] {}", stringify!($category).to_uppercase(), json_value);

        // 2. Push to frontend ring buffer
        let entry = $crate::models::log_collector::LogEntry {
            ts: chrono::Utc::now().to_rfc3339(),
            event: stringify!($category).to_lowercase(),
            pipeline: json_value.get("pipeline").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            ip: json_value.get("ip").and_then(|v| v.as_str()).map(|s| s.to_string()),
            country: json_value.get("country").and_then(|v| v.as_str()).map(|s| s.to_string()),
            rule_id: json_value.get("rule_id").and_then(|v| v.as_i64()).map(|n| n as i32),
            rule_name: json_value.get("rule_name").and_then(|v| v.as_str()).map(|s| s.to_string()),
            path: json_value.get("path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            method: json_value.get("method").and_then(|v| v.as_str()).map(|s| s.to_string()),
            query: json_value.get("query").and_then(|v| v.as_str()).map(|s| s.to_string()),
            ua: json_value.get("ua").and_then(|v| v.as_str()).map(|s| s.to_string()),
            fqdn: json_value.get("fqdn").and_then(|v| v.as_str()).map(|s| s.to_string()),
            referer: json_value.get("referer").and_then(|v| v.as_str()).map(|s| s.to_string()),
            status_code: json_value.get("status_code").and_then(|v| v.as_i64()).map(|n| n as i32),
            profile: json_value.get("profile").and_then(|v| v.as_str()).map(|s| s.to_string()),
            reason: json_value.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
        };
        $crate::models::log_collector::push_log(entry);
    }};
}
```

- [ ] **Step 2: Modify `backend/src/models/mod.rs`**

Add:
```rust
pub mod log_collector;
```

- [ ] **Step 3: Verify compilation**

Run: `cd /data/rust/shuul/backend && cargo check 2>&1 | head -20`

---

### Task 2: Backend — HTTP handler (log.rs)

**Files:**
- Create: `backend/src/http/log.rs`
- Modify: `backend/src/http/mod.rs` (add mod and pub use)

**Interfaces:**
- Produces: `log_router() -> Router<Arc<AppState>>`
- Consumes: `LOG_COLLECTOR` global

- [ ] **Step 1: Create `backend/src/http/log.rs`**

```rust
//! # Log viewer endpoint
//!
//! Provides access to the in-memory ring buffer for the frontend log viewer.
//! No persistence — all data is lost on restart.
//!
//! ## Endpoints
//!
//! - `GET /api/v1/logs` — Returns all log entries (client-side pagination).
//!   Optional query param `?event=block,report_ban` to filter by event type.
//! - `PUT /api/v1/logs/capacity` — Changes ring buffer capacity at runtime.
//!   Body: `{ "capacity": 5000 }`

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::models::{
    AppState, EmptyResponse,
    log_collector::{LOG_COLLECTOR, LogEntry},
};

pub fn log_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(list_logs))
        .route("/capacity", routing::put(set_capacity))
}

#[derive(Deserialize)]
struct LogFilter {
    event: Option<String>,
}

/// GET /api/v1/logs — Get all buffered log entries.
async fn list_logs(
    State(_app_state): State<Arc<AppState>>,
    Query(filter): Query<LogFilter>,
) -> impl IntoResponse {
    let entries = match LOG_COLLECTOR.lock() {
        Ok(collector) => {
            let all = collector.all();
            if let Some(event_filter) = filter.event {
                let events: Vec<&str> = event_filter.split(',').map(|s| s.trim()).collect();
                all.into_iter()
                    .filter(|e| events.contains(&e.event.as_str()))
                    .collect::<Vec<_>>()
            } else {
                all
            }
        },
        Err(_) => return EmptyResponse::create(StatusCode::INTERNAL_SERVER_ERROR, "Log collector poisoned"),
    };

    let capacity = LOG_COLLECTOR.lock().map(|c| c.capacity()).unwrap_or(0);
    let total = entries.len();

    Json(serde_json::json!({
        "status": 200,
        "data": {
            "entries": entries,
            "total": total,
            "capacity": capacity,
        }
    }))
}

#[derive(Deserialize)]
struct CapacityRequest {
    capacity: usize,
}

/// PUT /api/v1/logs/capacity — Update ring buffer capacity.
async fn set_capacity(
    State(_app_state): State<Arc<AppState>>,
    Json(req): Json<CapacityRequest>,
) -> impl IntoResponse {
    let new_cap = match req.capacity {
        1000 | 5000 | 10000 | 20000 => req.capacity,
        other => {
            return Json(serde_json::json!({
                "status": 400,
                "message": format!("Invalid capacity: {}. Valid values: 1000, 5000, 10000, 20000", other)
            }));
        },
    };

    match LOG_COLLECTOR.lock() {
        Ok(mut collector) => {
            collector.set_capacity(new_cap);
            Json(serde_json::json!({
                "status": 200,
                "data": {
                    "capacity": new_cap,
                    "entries": collector.len(),
                }
            }))
        },
        Err(_) => Json(serde_json::json!({
            "status": 500,
            "message": "Log collector poisoned"
        })),
    }
}
```

- [ ] **Step 2: Modify `backend/src/http/mod.rs`**

Add:
```rust
mod log;
// ...
pub use log::log_router;
```

- [ ] **Step 3: Verify compilation**

Run: `cd /data/rust/shuul/backend && cargo check 2>&1 | head -20`

---

### Task 3: Backend — Remove duplicate macros + wire up routes

**Files:**
- Modify: `backend/src/http/shuul.rs` (remove macro, import from models)
- Modify: `backend/src/http/report.rs` (remove macro, import from models)
- Modify: `backend/src/main.rs` (init LogCollector, add /logs route)

- [ ] **Step 1: Remove macro from `shuul.rs`**

In `backend/src/http/shuul.rs`:
- Delete the `should_log()` function (it's only used by the macro)
- Delete the `audit_log!` macro definition (lines 37-49)
- The macro is now imported via `crate::audit_log!` automatically since it's `#[macro_export]` in `log_collector.rs`

Note: `should_log()` is used by the macro itself, but also referenced directly in shuul.rs for conditions. Actually, looking at the code more carefully — `should_log` is used in `shuul.rs` at lines 107, 138, 167, 210, 268, 312, 343, 367. These are all the call sites of `audit_log!`. The `should_log` function is local to shuul.rs, so it stays.

Wait — the `audit_log!` macro currently uses `should_log` and `tracing::info!`. The new macro in `log_collector.rs` uses `tracing::info!` but not `should_log` — the `should_log` check is done *before* calling the macro in the current code. So this is fine: we keep `should_log` in both files and just use the macro from the shared location. We just need to remove the `audit_log!` macro definition from both files.

In `backend/src/http/shuul.rs`:
```rust
// DELETE these lines (37-49):
/// Macro for structured audit logging with visible category tag.
macro_rules! audit_log {
    ($category:expr, $($arg:tt)*) => {
        tracing::info!(
            "[{}] {}",
            $category.to_uppercase(),
            serde_json::json!({
                "event": $category,
                "ts": chrono::Utc::now().to_rfc3339(),
                $($arg)*
            })
        )
    };
}
```

The `audit_log!` calls (lines 108, 139, 168, 211, 269, 313, 344, 368) will use the shared macro from `crate::audit_log!` automatically (since `#[macro_export]` makes it available at crate root).

- [ ] **Step 2: Remove macro from `report.rs`**

Same deletion in `backend/src/http/report.rs`:
- Delete the `audit_log!` macro (lines 37-49)

- [ ] **Step 3: Modify `backend/src/main.rs`**

Add `/logs` to protected routes:

```rust
// In protected_routes section, add:
.nest("/logs", log_router())
```

And add `log_router` to the imports:
```rust
use http::{
    auth_router, ban_router, health_router, log_router, rate_limit_profile_router,
    report_router, require_auth, rule_router, settings_router, shuul_router,
    stats_router, template_router, util_router,
};
```

- [ ] **Step 4: Verify compilation**

Run: `cd /data/rust/shuul/backend && cargo check 2>&1 | head -30`

---

### Task 4: Frontend — LogsPage

**Files:**
- Create: `frontend/src/pages/admin/logs_page.tsx`
- Modify: `frontend/src/layouts/admin_layout.tsx` (add menu item)
- Modify: `frontend/src/App.tsx` (add route)

- [ ] **Step 1: Create `frontend/src/pages/admin/logs_page.tsx`**

The logs page follows the same class-component pattern as charts_page.tsx:

```tsx
import React from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";
import {
  Table, Tag, Button, Flex, Typography, Select, Switch, message, Card, Spin,
} from "antd";
import {
  EyeOutlined, ReloadOutlined, FilterOutlined,
} from "@ant-design/icons";
import { loadData } from "@/common/utils";
import { BASE_URL } from "@/constants";
import type { DebouncedFn } from "@/common/utils";

const { Text } = Typography;

// ── Event tag color map ──
const EVENT_COLORS: Record<string, string> = {
  safe_path: "green",
  trusted_ip: "cyan",
  trusted_ua: "geekblue",
  banned: "red",
  pass: "default",
  allow: "success",
  block: "error",
  log_only: "warning",
  report_received: "purple",
  report_match: "orange",
  report_block: "volcano",
  report_ban: "red",
  report_ok: "default",
  report_warn: "gold",
};
const DEFAULT_EVENT_COLOR = "default";

interface LogEntry {
  ts: string;
  event: string;
  pipeline: string;
  ip: string | null;
  country: string | null;
  rule_id: number | null;
  rule_name: string | null;
  path: string | null;
  method: string | null;
  query: string | null;
  ua: string | null;
  fqdn: string | null;
  referer: string | null;
  status_code: number | null;
  profile: string | null;
  reason: string | null;
}

interface LogResponse {
  entries: LogEntry[];
  total: number;
  capacity: number;
}

interface Props {
  navigate: any;
  t: any;
}

interface State {
  loading: boolean;
  entries: LogEntry[];
  capacity: number;
  total: number;
  filterEvent: string;
  autoRefresh: boolean;
}

const CAPACITY_OPTIONS = [1000, 5000, 10000, 20000];

class InnerPage extends React.Component<Props, State> {
  private refreshTimer: ReturnType<typeof setInterval> | null = null;

  constructor(props: Props) {
    super(props);
    this.state = {
      loading: true,
      entries: [],
      capacity: 1000,
      total: 0,
      filterEvent: "",
      autoRefresh: false,
    };
  }

  loadLogs = async () => {
    try {
      const params = new Map<string, string>();
      if (this.state.filterEvent) {
        params.set("event", this.state.filterEvent);
      }
      const res = await loadData<LogResponse>("logs", params);
      if (res.status === 200 && res.data) {
        this.setState({
          entries: (res.data as any).entries || [],
          total: (res.data as any).total || 0,
          capacity: (res.data as any).capacity || 1000,
          loading: false,
        });
      } else {
        this.setState({ loading: false });
      }
    } catch (e) {
      console.error("Failed to load logs:", e);
      this.setState({ loading: false });
    }
  };

  componentDidMount = async () => {
    await this.loadLogs();
  };

  componentWillUnmount = () => {
    this.stopAutoRefresh();
  };

  startAutoRefresh = () => {
    this.stopAutoRefresh();
    this.refreshTimer = setInterval(() => {
      this.loadLogs();
    }, 3000);
  };

  stopAutoRefresh = () => {
    if (this.refreshTimer) {
      clearInterval(this.refreshTimer);
      this.refreshTimer = null;
    }
  };

  handleAutoRefreshChange = (checked: boolean) => {
    this.setState({ autoRefresh: checked });
    if (checked) {
      this.startAutoRefresh();
    } else {
      this.stopAutoRefresh();
    }
  };

  handleCapacityChange = async (value: number) => {
    const token = localStorage.getItem("token");
    try {
      const response = await fetch(`${BASE_URL}/api/v1/logs/capacity`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: JSON.stringify({ capacity: value }),
      });
      if (response.ok) {
        this.setState({ capacity: value });
        message.success(`Buffer capacity set to ${value}`);
      } else {
        if (response.status === 401) {
          window.location.href = "/login";
          return;
        }
        const err = await response.json();
        message.error(`Failed to set capacity: ${err.message || "Unknown"}`);
      }
    } catch (e: any) {
      message.error(`Error: ${e.message}`);
    }
  };

  formatTime = (iso: string) => {
    const d = new Date(iso);
    return `${String(d.getDate()).padStart(2, "0")}/${String(d.getMonth() + 1).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}:${String(d.getSeconds()).padStart(2, "0")}`;
  };

  getEventColor = (event: string) => EVENT_COLORS[event] || DEFAULT_EVENT_COLOR;

  setFilter = (event: string) => {
    this.setState(
      { filterEvent: this.state.filterEvent === event ? "" : event },
      () => this.loadLogs(),
    );
  };

  renderLogDetail = (record: LogEntry) => {
    const detail = { ...record };
    return (
      <pre style={{ fontSize: 12, maxHeight: 300, overflow: "auto", margin: 0 }}>
        {JSON.stringify(detail, null, 2)}
      </pre>
    );
  };

  render = () => {
    const columns = [
      {
        title: "Timestamp",
        dataIndex: "ts",
        key: "ts",
        width: 160,
        render: (_: any, record: LogEntry) => this.formatTime(record.ts),
      },
      {
        title: "Event",
        dataIndex: "event",
        key: "event",
        width: 130,
        render: (_: any, record: LogEntry) => (
          <Tag color={this.getEventColor(record.event)}>{record.event}</Tag>
        ),
      },
      {
        title: "Pipeline",
        dataIndex: "pipeline",
        key: "pipeline",
        width: 80,
        render: (_: any, record: LogEntry) => (
          <Tag color={record.pipeline === "jail" ? "orange" : "blue"}>
            {record.pipeline || "-"}
          </Tag>
        ),
      },
      {
        title: "IP",
        dataIndex: "ip",
        key: "ip",
        width: 140,
        render: (_: any, record: LogEntry) => record.ip || "-",
      },
      {
        title: "Country",
        dataIndex: "country",
        key: "country",
        width: 80,
        render: (_: any, record: LogEntry) => record.country || "-",
      },
      {
        title: "Rule",
        dataIndex: "rule_name",
        key: "rule_name",
        width: 180,
        ellipsis: true,
        render: (_: any, record: LogEntry) => record.rule_name || "-",
      },
      {
        title: "Path",
        dataIndex: "path",
        key: "path",
        width: 250,
        ellipsis: true,
        render: (_: any, record: LogEntry) => record.path || "-",
      },
      {
        title: "Method",
        dataIndex: "method",
        key: "method",
        width: 80,
        render: (_: any, record: LogEntry) => record.method ? (
          <Tag color="magenta">{record.method}</Tag>
        ) : "-",
      },
      {
        title: "Status",
        dataIndex: "status_code",
        key: "status_code",
        width: 70,
        render: (_: any, record: LogEntry) =>
          record.status_code ? (
            <Tag color={record.status_code >= 400 ? "red" : "green"}>
              {record.status_code}
            </Tag>
          ) : "-",
      },
    ];

    // Collect unique event types for filter buttons
    const eventTypes = Array.from(
      new Set(this.state.entries.map((e) => e.event)),
    ).sort();

    return (
      <Flex vertical gap="middle" style={{ padding: 24 }}>
        {/* Header: title, capacity, auto-refresh, refresh button */}
        <Flex justify="space-between" align="center" wrap gap="middle">
          <Flex gap="small" align="center">
            <EyeOutlined style={{ fontSize: 20 }} />
            <Text strong style={{ fontSize: 18 }}>
              Log Viewer
            </Text>
            <Tag>{this.state.total} entries</Tag>
            <Tag color="blue">Buffer: {this.state.capacity}</Tag>
          </Flex>
          <Flex gap="small" align="center">
            <Text>Buffer capacity:</Text>
            <Select
              value={this.state.capacity}
              onChange={this.handleCapacityChange}
              options={CAPACITY_OPTIONS.map((c) => ({
                value: c,
                label: c.toLocaleString(),
              }))}
              style={{ width: 120 }}
            />
            <Text>Auto-refresh:</Text>
            <Switch
              checked={this.state.autoRefresh}
              onChange={this.handleAutoRefreshChange}
            />
            <Button
              icon={<ReloadOutlined />}
              onClick={() => this.loadLogs()}
              loading={this.state.loading}
            >
              Refresh
            </Button>
          </Flex>
        </Flex>

        {/* Event type filter buttons */}
        <Flex wrap gap="small" align="center">
          <Text type="secondary">Filter by event:</Text>
          <Button
            size="small"
            type={this.state.filterEvent === "" ? "primary" : "default"}
            onClick={() => this.setFilter("")}
          >
            All
          </Button>
          {eventTypes.map((event) => (
            <Button
              key={event}
              size="small"
              type={this.state.filterEvent === event ? "primary" : "default"}
              onClick={() => this.setFilter(event)}
            >
              {event}
            </Button>
          ))}
        </Flex>

        {/* Table */}
        <Table<LogEntry>
          dataSource={this.state.entries}
          columns={columns}
          rowKey={(record, index) => `${record.ts}-${index}`}
          loading={this.state.loading}
          size="small"
          pagination={{
            pageSize: 50,
            showSizeChanger: true,
            pageSizeOptions: ["20", "50", "100"],
            showTotal: (total, range) =>
              `${range[0]}-${range[1]} of ${total} entries`,
          }}
          expandable={{
            expandedRowRender: this.renderLogDetail,
          }}
          scroll={{ x: 1200 }}
          locale={{
            emptyText: "No log entries yet. Logs appear when traffic flows through the WAF/Jail pipelines.",
          }}
        />
      </Flex>
    );
  };
}

export default function LogsPage() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  return <InnerPage navigate={navigate} t={t} />;
}
```

- [ ] **Step 2: Modify `frontend/src/layouts/admin_layout.tsx`**

Add import:
```tsx
import { ..., EyeOutlined } from "@ant-design/icons";
```

Add menu item after Charts (key "6"):
```tsx
getItem("Logs", "7", <EyeOutlined />),
```

And shift the remaining items' keys:
```tsx
// Old:
// 7 -> /admin/settings
// New:
// 8 -> /admin/settings
```

Update `navigations`:
```tsx
const navigations: { [key: string]: string } = {
  1: "/admin/dashboard",
  2: "/admin/rules",
  3: "/admin/rate-limit-profiles",
  4: "/admin/bans",
  5: "/admin/templates",
  6: "/admin/charts",
  7: "/admin/logs",
  8: "/admin/settings",
};
```

Update `items`:
```tsx
const items: MenuItem[] = [
  getItem("Dashboard", "1", <HomeOutlined />),
  getItem("Rules", "2", <OrderedListOutlined />),
  getItem("Rate Limit Profiles", "3", <RocketOutlined />),
  getItem("Bans", "4", <StopOutlined />),
  getItem("Templates", "5", <AppstoreOutlined />),
  getItem("Charts", "6", <PieChartOutlined />),
  getItem("Logs", "7", <EyeOutlined />),
  getItem("Settings", "8", <SettingOutlined />),
];
```

- [ ] **Step 3: Modify `frontend/src/App.tsx`**

Add import:
```tsx
const LogsPage = lazy(() => import("@/pages/admin/logs_page"));
```

Add route (after `charts`):
```tsx
<Route path="logs" element={<LogsPage />} />
```

- [ ] **Step 4: Verify frontend compiles**

Run: `cd /data/rust/shuul/frontend && npx tsc --noEmit 2>&1 | head -30`

---

### Task 5: Full verification

- [ ] **Step 1: Build backend**

Run: `cd /data/rust/shuul/backend && cargo build 2>&1 | tail -10`

- [ ] **Step 2: Build frontend**

Run: `cd /data/rust/shuul/frontend && npx vite build 2>&1 | tail -10`

- [ ] **Step 3: End-to-end check**

Start the backend, call:
```
curl -s http://localhost:3000/api/v1/logs | head -20
curl -s -X PUT http://localhost:3000/api/v1/logs/capacity -H 'Content-Type: application/json' -d '{"capacity":5000}' | head -20
```