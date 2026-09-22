import React from "react";
import {
  Card,
  Form,
  InputNumber,
  Select,
  Button,
  Typography,
  message,
  Flex,
  Tabs,
} from "antd";
import type { TabsProps } from "antd";
import { BASE_URL } from "@/constants";

const { Title } = Typography;

interface Settings {
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
    } catch (_error) {
      message.error("Error saving settings");
    } finally {
      this.setState({ saving: false });
    }
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
              onFinish={this.handleSave}
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
                  Save Settings
                </Button>
              </Form.Item>
            </Form>
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