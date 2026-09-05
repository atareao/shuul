import React from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";
import {
  Flex,
  Typography,
  Table,
  Tag,
  Select,
  Switch,
  Button,
  Spin,
  Card,
  message,
} from "antd";
import { EyeOutlined, ReloadOutlined } from "@ant-design/icons";
import type { ColumnsType } from "antd/es/table";
import { loadData } from "@/common/utils";
import { BASE_URL } from "@/constants";
import ModeContext from "@/components/mode_context";

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

interface LogsResponse {
  entries: LogEntry[];
  total: number;
  capacity: number;
}

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
  report_skip: "gold",
};

const CAPACITY_OPTIONS = [1000, 5000, 10000, 20000];

interface Props {
  navigate: any;
  t: any;
  isDarkMode: boolean;
}

interface State {
  loading: boolean;
  error: boolean;
  entries: LogEntry[];
  total: number;
  capacity: number;
  hiddenEvents: string[];
  autoRefresh: boolean;
}

function formatTimestamp(ts: string): string {
  const d = new Date(ts);
  const dd = String(d.getDate()).padStart(2, "0");
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const hh = String(d.getHours()).padStart(2, "0");
  const min = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return `${dd}/${mm} ${hh}:${min}:${ss}`;
}

export class InnerPage extends React.Component<Props, State> {
  private pollTimer: ReturnType<typeof setInterval> | null = null;

  constructor(props: Props) {
    super(props);
    this.state = {
      loading: true,
      error: false,
      entries: [],
      total: 0,
      capacity: 1000,
      hiddenEvents: [],
      autoRefresh: false,
    };
  }

  refreshData = async () => {
    try {
      this.setState({ loading: true, error: false });
      const res = await loadData<LogsResponse>("logs");
      if (res.status === 200 && res.data) {
        this.setState({
          loading: false,
          entries: (res.data.entries || []).reverse(),
          total: res.data.total,
          capacity: res.data.capacity,
        });
      } else {
        this.setState({ loading: false, error: true });
      }
    } catch (err) {
      console.error("Failed to load logs:", err);
      this.setState({ loading: false, error: true });
    }
  };

  updateCapacity = async (newCapacity: number) => {
    const token = localStorage.getItem("token");
    try {
      const response = await fetch(`${BASE_URL}/api/v1/logs/capacity`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: JSON.stringify({ capacity: newCapacity }),
      });
      if (response.status === 401) {
        window.location.href = "/login";
        return;
      }
      const json = await response.json();
      if (response.ok && json.data) {
        this.setState({
          capacity: json.data.capacity,
        });
        message.success(`Buffer capacity set to ${json.data.capacity}`);
      } else {
        message.error(json.message || "Failed to update capacity");
      }
    } catch (err) {
      console.error("Failed to update capacity:", err);
      message.error("Failed to update capacity");
    }
  };

  toggleEventFilter = (event: string) => {
    this.setState((prevState) => {
      const hiddenEvents = prevState.hiddenEvents.includes(event)
        ? prevState.hiddenEvents.filter((e) => e !== event)
        : [...prevState.hiddenEvents, event];
      return { hiddenEvents };
    });
  };

  showAllEvents = () => {
    this.setState({ hiddenEvents: [] });
  };

  toggleAutoRefresh = (checked: boolean) => {
    this.setState({ autoRefresh: checked });
    if (checked) {
      this.pollTimer = setInterval(() => {
        this.refreshData();
      }, 3000);
    } else {
      if (this.pollTimer) {
        clearInterval(this.pollTimer);
        this.pollTimer = null;
      }
    }
  };

  componentDidMount = async () => {
    try {
      await this.refreshData();
    } catch (err) {
      console.error("Failed to load logs on mount:", err);
      this.setState({ loading: false, error: true });
    }
  };

  componentWillUnmount = () => {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  };

  getUniqueEvents = (): string[] => {
    const events = new Set<string>();
    for (const entry of this.state.entries) {
      if (entry.event) {
        events.add(entry.event);
      }
    }
    return Array.from(events).sort();
  };

  render = () => {
    const {
      loading,
      error,
      entries,
      total,
      capacity,
      hiddenEvents,
      autoRefresh,
    } = this.state;

    const columns: ColumnsType<LogEntry> = [
      {
        title: "Timestamp",
        dataIndex: "ts",
        key: "ts",
        width: 160,
        render: (ts: string) => formatTimestamp(ts),
      },
      {
        title: "Event",
        dataIndex: "event",
        key: "event",
        width: 130,
        render: (event: string) => (
          <Tag color={EVENT_COLORS[event] || "default"}>{event}</Tag>
        ),
      },
      {
        title: "Pipe",
        dataIndex: "pipeline",
        key: "pipeline",
        width: 80,
        render: (pipeline: string) => {
          if (!pipeline) return null;
          return (
            <Tag color={pipeline === "waf" ? "blue" : "orange"}>{pipeline}</Tag>
          );
        },
      },
      {
        title: "IP",
        dataIndex: "ip",
        key: "ip",
        width: 140,
        render: (ip: string | null) => ip || "-",
      },
      {
        title: "Country",
        dataIndex: "country",
        key: "country",
        width: 80,
        render: (country: string | null) => country || "-",
      },
      {
        title: "Rule",
        dataIndex: "rule_name",
        key: "rule_name",
        width: 180,
        ellipsis: true,
        render: (rule_name: string | null) => rule_name || "-",
      },
      {
        title: "Path",
        dataIndex: "path",
        key: "path",
        width: 250,
        ellipsis: true,
        render: (path: string | null) => path || "-",
      },
      {
        title: "Method",
        dataIndex: "method",
        key: "method",
        width: 80,
        render: (method: string | null) =>
          method ? <Tag color="magenta">{method}</Tag> : null,
      },
      {
        title: "Status",
        dataIndex: "status_code",
        key: "status_code",
        width: 70,
        render: (status_code: number | null) => {
          if (status_code === null || status_code === undefined) return null;
          return (
            <Tag color={status_code >= 400 ? "red" : "green"}>
              {status_code}
            </Tag>
          );
        },
      },
    ];

    const uniqueEvents = this.getUniqueEvents();

    const filteredEntries =
      hiddenEvents.length === 0
        ? entries
        : entries.filter(
            (entry) => entry.event && !hiddenEvents.includes(entry.event),
          );

    if (loading && entries.length === 0) {
      return (
        <Flex
          vertical
          justify="center"
          align="center"
          style={{ minHeight: 400 }}
        >
          <Spin size="large" />
        </Flex>
      );
    }

    if (error && entries.length === 0) {
      return (
        <Flex
          vertical
          justify="center"
          align="center"
          style={{ minHeight: 400 }}
        >
          <Card style={{ width: 400, textAlign: "center" }}>
            <Typography.Text type="danger" style={{ fontSize: 16 }}>
              Failed to load logs
            </Typography.Text>
          </Card>
        </Flex>
      );
    }

    return (
      <Flex vertical gap="middle" style={{ padding: 24 }}>
        {/* Header */}
        <Flex justify="space-between" align="center" wrap gap="small">
          <Flex align="center" gap="small">
            <EyeOutlined style={{ fontSize: 24 }} />
            <Typography.Title level={4} style={{ margin: 0 }}>
              Log Viewer
            </Typography.Title>
            <Tag>{total} entries</Tag>
            <Tag>Buffer: {capacity}</Tag>
          </Flex>
          <Flex align="center" gap="middle" wrap>
            <Flex align="center" gap="small">
              <Typography.Text>Buffer:</Typography.Text>
              <Select
                value={capacity}
                onChange={(value) => this.updateCapacity(value)}
                options={CAPACITY_OPTIONS.map((c) => ({
                  value: c,
                  label: `${c}`,
                }))}
                style={{ width: 100 }}
                size="small"
              />
            </Flex>
            <Flex align="center" gap="small">
              <Typography.Text>Auto-refresh</Typography.Text>
              <Switch
                checked={autoRefresh}
                onChange={this.toggleAutoRefresh}
                size="small"
              />
            </Flex>
            <Button
              icon={<ReloadOutlined />}
              onClick={() => this.refreshData()}
              loading={loading}
              size="small"
            >
              Refresh
            </Button>
          </Flex>
        </Flex>

        {/* Event filter toggles */}
        <Flex wrap gap="small">
          <Button
            size="small"
            type={hiddenEvents.length === 0 ? "primary" : "default"}
            onClick={this.showAllEvents}
          >
            All
          </Button>
          {uniqueEvents.map((event) => (
            <Button
              key={event}
              size="small"
              type={hiddenEvents.includes(event) ? "default" : "primary"}
              onClick={() => this.toggleEventFilter(event)}
            >
              {event}
            </Button>
          ))}
        </Flex>

        {/* Table */}
        <Table<LogEntry>
          size="small"
          dataSource={filteredEntries}
          columns={columns}
          rowKey={(record, index) => `${record.ts}-${index}`}
          scroll={{ x: 1200 }}
          loading={loading && entries.length > 0}
          pagination={{
            pageSize: 50,
            pageSizeOptions: ["20", "50", "100"],
            showSizeChanger: true,
            showTotal: (total, range) =>
              `${range[0]}-${range[1]} of ${total} entries`,
          }}
          expandable={{
            expandedRowRender: (record) => (
              <pre
                style={{
                  maxHeight: 300,
                  overflow: "auto",
                  margin: 0,
                  fontSize: 12,
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-all",
                }}
              >
                {JSON.stringify(record, null, 2)}
              </pre>
            ),
          }}
          locale={{
            emptyText:
              "No log entries yet. Logs appear when traffic flows through the WAF/Jail pipelines.",
          }}
        />
      </Flex>
    );
  };
}

export default function LogsPage() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  return (
    <ModeContext.Consumer>
      {({ isDarkMode }) => {
        return <InnerPage navigate={navigate} t={t} isDarkMode={isDarkMode} />;
      }}
    </ModeContext.Consumer>
  );
}
