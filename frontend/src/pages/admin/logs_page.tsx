import React from "react";
import { useNavigate } from "react-router";
import type { NavigateFunction } from "react-router";
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

const LOGS_PAGE_STORAGE_KEY = "logsPage";

interface PersistedLogsState {
  hiddenEvents: string[];
  autoRefresh: boolean;
}

function loadPersistedLogsState(): PersistedLogsState {
  try {
    const raw = localStorage.getItem(LOGS_PAGE_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        hiddenEvents: Array.isArray(parsed.hiddenEvents) ? parsed.hiddenEvents : [],
        autoRefresh: typeof parsed.autoRefresh === "boolean" ? parsed.autoRefresh : true,
      };
    }
  } catch {
  // ignore parse errors
  }
  return { hiddenEvents: [], autoRefresh: true };
}

function savePersistedLogsState(hiddenEvents: string[], autoRefresh: boolean): void {
  try {
    localStorage.setItem(
      LOGS_PAGE_STORAGE_KEY,
      JSON.stringify({ hiddenEvents, autoRefresh })
    );
  } catch {
  // ignore storage errors
  }
}

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
  denied: "error",
  banned: "red",
  admitted: "success",
  cleared: "default",
  registered: "orange",
  sanctioned: "volcano",
};

const CAPACITY_OPTIONS = [1000, 5000, 10000, 20000];

/**
 * Props públicas de `InnerPage`. `t` se declara con la firma mínima que usa la
 * página (`(key: string) => string`) en lugar del `TFunction` de i18next para
 * que tanto el wrapper (`useTranslation`) como los tests puedan inyectarla.
 */
interface Props {
  navigate: NavigateFunction;
  t: (key: string) => string;
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
  /**
   * Entradas por página del visor. Vive en el estado —y no como prop constante
   * del `Table`— porque pasar `pageSize` convierte esa parte de la `Pagination`
   * de antd en controlada: con un valor fijo el selector de tamaño no podría
   * cambiarlo. No se persiste en `localStorage`: el estado persistido sigue
   * siendo `{hiddenEvents, autoRefresh}`.
   */
  pageSize: number;
}

export class InnerPage extends React.Component<Props, State> {
  private pollTimer: ReturnType<typeof setInterval> | null = null;

  constructor(props: Props) {
    super(props);
    const persisted = loadPersistedLogsState();
    this.state = {
      loading: true,
      error: false,
      entries: [],
      total: 0,
      capacity: 1000,
      hiddenEvents: persisted.hiddenEvents,
      autoRefresh: persisted.autoRefresh,
      pageSize: 10,
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
        window.location.href = "/";
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
    }, () => {
      savePersistedLogsState(this.state.hiddenEvents, this.state.autoRefresh);
    });
  };

  showAllEvents = () => {
    this.setState({ hiddenEvents: [] }, () => {
      savePersistedLogsState([], this.state.autoRefresh);
    });
  };

  toggleAutoRefresh = (checked: boolean) => {
    this.setState({ autoRefresh: checked }, () => {
      savePersistedLogsState(this.state.hiddenEvents, checked);
    });
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
    // `localStorage` es la única fuente de verdad del estado: la query string de
    // la URL no se lee ni se escribe.
    try {
      await this.refreshData();
      if (this.state.autoRefresh) {
        this.pollTimer = setInterval(() => {
          this.refreshData();
        }, 3000);
      }
    } catch (err) {
      console.error("Failed to load logs on mount:", err);
      this.setState({ loading: false, error: true });
    }
  };

  componentWillUnmount = () => {
    savePersistedLogsState(this.state.hiddenEvents, this.state.autoRefresh);
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
        title: "FQDN",
        dataIndex: "fqdn",
        key: "fqdn",
        width: 200,
        ellipsis: true,
        render: (fqdn: string | null) => fqdn || "-",
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
            pageSize: this.state.pageSize,
            pageSizeOptions: ["10", "50", "100"],
            showSizeChanger: true,
            // `pageSize` sale del estado para que el selector de tamaño pueda
            // fijarlo (un valor constante en el `Table` lo dejaría controlado y
            // el selector sería inerte). La `Pagination` de antd emite
            // `onChange` también al cambiar el tamaño —llama a
            // `onShowSizeChange` y a `onChange` con el nuevo tamaño—, así que
            // basta con esta rama. `current` no se controla: el visor no
            // necesita resetear a la página 1 y antd recorta la página si queda
            // fuera de rango.
            onChange: (_page, pageSize) => {
              // Sólo se confirma el tamaño: `current` no se controla, y antd
              // recorta la página si el conjunto encogido la deja fuera de rango.
              if (pageSize !== this.state.pageSize) this.setState({ pageSize });
            },
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
