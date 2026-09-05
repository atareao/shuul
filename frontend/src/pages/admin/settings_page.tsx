import React from "react";
import {
  Card,
  Form,
  InputNumber,
  Input,
  Select,
  Button,
  Typography,
  message,
  Flex,
  Tabs,
  Tag,
} from "antd";
import type { TabsProps } from "antd";
import { PlusOutlined, MinusCircleOutlined } from "@ant-design/icons";
import { BASE_URL } from "@/constants";

const { Title, Text } = Typography;

interface Settings {
  safe_paths: string[];
  trusted_ips: string[];
  trusted_user_agents: string[];
  default_rule_mode: string;
  log_retention_days: number;
  log_all_requests: string;
}

interface State {
  settings: Settings | null;
  loading: boolean;
  saving: boolean;
}

const DEFAULT_SETTINGS: Settings = {
  safe_paths: [],
  trusted_ips: [],
  trusted_user_agents: [],
  default_rule_mode: "enforce",
  log_retention_days: 30,
  log_all_requests: "all",
};

export default class SettingsPage extends React.Component<{}, State> {
  constructor(props: {}) {
    super(props);
    this.state = {
      settings: null,
      loading: true,
      saving: false,
    };
  }

  componentDidMount = async () => {
    await this.loadSettings();
  };

  loadSettings = async () => {
    this.setState({ loading: true });
    const token = localStorage.getItem("token");
    try {
      const response = await fetch(`${BASE_URL}/api/v1/settings`, {
        headers: {
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
      });
      if (response.status === 401) {
        window.location.href = "/login";
        return;
      }
      const json = await response.json();
      if (response.ok && json.data) {
        this.setState({ settings: json.data });
      }
    } catch (error) {
      console.error("Error loading settings:", error);
    } finally {
      this.setState({ loading: false });
    }
  };

  handleSave = async (values: Record<string, any>) => {
    this.setState({ saving: true });
    const token = localStorage.getItem("token");
    try {
      const response = await fetch(`${BASE_URL}/api/v1/settings`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: JSON.stringify(values),
      });
      const json = await response.json();
      if (response.ok) {
        message.success("Settings saved");
        this.setState({ settings: json.data });
      } else {
        if (response.status === 401) {
          window.location.href = "/login";
          return;
        }
        message.error(json.message || "Failed to save settings");
      }
    } catch (error) {
      message.error("Error saving settings");
    } finally {
      this.setState({ saving: false });
    }
  };

  private updateListField = (
    key: "safe_paths" | "trusted_ips" | "trusted_user_agents",
    index: number,
    value: string,
  ) => {
    const settings = { ...this.state.settings } as Settings;
    settings[key][index] = value;
    this.setState({ settings });
  };

  private addListField = (
    key: "safe_paths" | "trusted_ips" | "trusted_user_agents",
  ) => {
    const settings = { ...this.state.settings } as Settings;
    settings[key] = [...settings[key], ""];
    this.setState({ settings });
  };

  private removeListField = (
    key: "safe_paths" | "trusted_ips" | "trusted_user_agents",
    index: number,
  ) => {
    const settings = { ...this.state.settings } as Settings;
    settings[key] = settings[key].filter((_, i) => i !== index);
    this.setState({ settings });
  };

  private renderListEditor = (
    key: "safe_paths" | "trusted_ips" | "trusted_user_agents",
    placeholder: string,
    helpText: string,
  ) => {
    const items = this.state.settings?.[key] ?? [];
    return (
      <Flex vertical gap="small">
        <Text type="secondary">{helpText}</Text>
        {items.map((item, index) => (
          <Flex key={index} align="center" gap="small">
            <Input
              style={{ flex: 1 }}
              value={item}
              placeholder={placeholder}
              onChange={(e) => this.updateListField(key, index, e.target.value)}
            />
            <Button
              type="text"
              danger
              icon={<MinusCircleOutlined />}
              onClick={() => this.removeListField(key, index)}
            />
          </Flex>
        ))}
        <Button
          type="dashed"
          onClick={() => this.addListField(key)}
          icon={<PlusOutlined />}
          style={{ width: "fit-content" }}
        >
          Add
        </Button>
      </Flex>
    );
  };

  private handleGeneralSave = (values: {
    default_rule_mode: string;
    log_retention_days: number;
    log_all_requests: string;
  }) => {
    this.handleSave(values);
  };

  private handleListSave = (
    key: "safe_paths" | "trusted_ips" | "trusted_user_agents",
  ) => {
    const values: Record<string, any> = {};
    values[key] =
      this.state.settings?.[key]?.filter((s) => s.trim() !== "") ?? [];
    this.handleSave(values);
  };

  render() {
    if (this.state.loading) {
      return (
        <Flex justify="center" align="center" style={{ minHeight: 200 }}>
          <Title level={4}>Loading...</Title>
        </Flex>
      );
    }

    const settings = this.state.settings || DEFAULT_SETTINGS;

    const tabItems: TabsProps["items"] = [
      {
        key: "general",
        label: "General",
        children: (
          <Card>
            <Form
              layout="vertical"
              onFinish={this.handleGeneralSave}
              initialValues={{
                default_rule_mode: settings.default_rule_mode,
                log_retention_days: settings.log_retention_days,
                log_all_requests: settings.log_all_requests,
              }}
            >
              <Form.Item
                label="Default Rule Mode"
                name="default_rule_mode"
                help="Default mode for new rules created from templates"
              >
                <Select
                  options={[
                    { value: "enforce", label: "Enforce" },
                    { value: "log_only", label: "Log Only" },
                    { value: "off", label: "Off" },
                  ]}
                  style={{ width: 200 }}
                />
              </Form.Item>
              <Form.Item
                label="Log Retention (days)"
                name="log_retention_days"
                rules={[
                  { required: true, message: "Please set retention days" },
                  {
                    type: "number",
                    min: 1,
                    max: 365,
                    message: "Must be between 1 and 365",
                  },
                ]}
              >
                <InputNumber min={1} max={365} style={{ width: 200 }} />
              </Form.Item>
              <Form.Item
                label="Log Level"
                name="log_all_requests"
                help="Controls which events are logged. 'All' logs everything. 'Pass Only' logs only requests that pass without matching any rule. 'Audit Only' logs only blocks, bans, and enforcement actions."
              >
                <Select
                  options={[
                    { value: "all", label: "All" },
                    { value: "pass", label: "Pass Only" },
                    { value: "audit", label: "Audit Only" },
                  ]}
                  style={{ width: 200 }}
                />
              </Form.Item>
              <Form.Item>
                <Button
                  type="primary"
                  htmlType="submit"
                  loading={this.state.saving}
                >
                  Save General Settings
                </Button>
              </Form.Item>
            </Form>
          </Card>
        ),
      },
      {
        key: "safe-paths",
        label: "Safe Paths",
        children: (
          <Card
            title="Safe Paths"
            extra={
              <Button
                type="primary"
                loading={this.state.saving}
                onClick={() => this.handleListSave("safe_paths")}
              >
                Save Safe Paths
              </Button>
            }
          >
            {this.renderListEditor(
              "safe_paths",
              "e.g. ^/api/health$",
              "Requests matching these regex patterns will be allowed without any filtering. One pattern per line.",
            )}
            {settings.safe_paths.length > 0 && (
              <Flex wrap gap="small" style={{ marginTop: 16 }}>
                <Text strong style={{ width: "100%" }}>
                  Current patterns:
                </Text>
                {settings.safe_paths.map((p, i) => (
                  <Tag key={i} color="blue">
                    {p}
                  </Tag>
                ))}
              </Flex>
            )}
          </Card>
        ),
      },
      {
        key: "trusted-ips",
        label: "Trusted IPs",
        children: (
          <Card
            title="Trusted IPs"
            extra={
              <Button
                type="primary"
                loading={this.state.saving}
                onClick={() => this.handleListSave("trusted_ips")}
              >
                Save Trusted IPs
              </Button>
            }
          >
            {this.renderListEditor(
              "trusted_ips",
              "e.g. 10.0.0.0/8",
              "IPs or CIDR ranges that will bypass all filtering. One per line.",
            )}
            {settings.trusted_ips.length > 0 && (
              <Flex wrap gap="small" style={{ marginTop: 16 }}>
                <Text strong style={{ width: "100%" }}>
                  Current ranges:
                </Text>
                {settings.trusted_ips.map((ip, i) => (
                  <Tag key={i} color="green">
                    {ip}
                  </Tag>
                ))}
              </Flex>
            )}
          </Card>
        ),
      },
      {
        key: "trusted-uas",
        label: "Trusted User Agents",
        children: (
          <Card
            title="Trusted User Agents"
            extra={
              <Button
                type="primary"
                loading={this.state.saving}
                onClick={() => this.handleListSave("trusted_user_agents")}
              >
                Save Trusted UAs
              </Button>
            }
          >
            {this.renderListEditor(
              "trusted_user_agents",
              "e.g. ^kube-probe",
              "User-Agent regex patterns that will bypass all filtering. One per line.",
            )}
            {settings.trusted_user_agents.length > 0 && (
              <Flex wrap gap="small" style={{ marginTop: 16 }}>
                <Text strong style={{ width: "100%" }}>
                  Current patterns:
                </Text>
                {settings.trusted_user_agents.map((ua, i) => (
                  <Tag key={i} color="purple">
                    {ua}
                  </Tag>
                ))}
              </Flex>
            )}
          </Card>
        ),
      },
    ];

    return (
      <Flex vertical gap="middle" style={{ maxWidth: 800, margin: "0 auto" }}>
        <Tabs defaultActiveKey="general" items={tabItems} size="large" />
      </Flex>
    );
  }
}
