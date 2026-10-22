import React from "react";
import {
  Table,
  Tag,
  Button,
  Typography,
  Flex,
  Row,
  Col,
  message,
  Input,
  InputNumber,
  Modal,
  Select,
  Spin,
  Empty,
  Switch,
} from "antd";
import type { TableColumnsType } from "antd";
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  ThunderboltOutlined,
  SearchOutlined,
  LockOutlined,
} from "@ant-design/icons";
import type { RuleTemplate, TemplatesResponse } from "@/models/template";
import type Rule from "@/models/rule";
import { loadData } from "@/common/utils";
import { BASE_URL } from "@/constants";

const { Text } = Typography;

/**
 * Filtros que un template aporta tal cual a la nueva regla. Se reutiliza el
 * modelo `RuleTemplate`, donde ya están declarados como `string | null`
 * (los filtros vacíos viajan como `null` al backend).
 */
type TemplateFilterFields = Pick<
  RuleTemplate,
  | "path"
  | "query"
  | "country_code"
  | "ip_address"
  | "protocol"
  | "city_name"
  | "country_name"
  | "user_agent"
  | "method"
  | "referer"
  | "content_type"
  | "accept_language"
  | "x_request_id"
  | "is_tor"
>;

/**
 * Cuerpo de `POST /api/v1/rules` al aplicar un template: el subconjunto de
 * campos de `Rule` que envía el formulario (`fqdn` y `active` vienen del
 * formulario, no del template).
 */
type RuleCreatePayload = Pick<
  Rule,
  | "name"
  | "description"
  | "mode"
  | "weight"
  | "allow"
  | "pipeline"
  | "rate_limit_profile_id"
> &
  TemplateFilterFields & {
    fqdn: string | null;
    active: boolean;
  };

const SEVERITY_COLORS: Record<string, string> = {
  critico: "red",
  alto: "orange",
  medio: "gold",
  bajo: "green",
};

const SEVERITY_LABELS: Record<string, string> = {
  critico: "🔥 Crítico",
  alto: "🔴 Alto",
  medio: "🟡 Medio",
  bajo: "🟢 Bajo",
};

const CATEGORY_LABELS: Record<string, string> = {
  wordpress: "WordPress",
  joomla: "Joomla",
  drupal: "Drupal",
  laravel: "Laravel",
  paneles: "Paneles de administración",
  api: "API",
  servidores: "Servidores de aplicaciones",
  seguridad: "Seguridad general",
  bots: "Bots y scrapers",
  geo: "Geo",
  cms: "CMS",
  infra: "Infraestructura",
  probes: "Probes de seguridad",
  webmail: "Webmail",
};

/** Filtros de una plantilla, en el orden en que se muestran al expandir la fila. */
const FILTER_DETAIL_FIELDS: { key: TemplateFilterFields; label: string }[] = [
  { key: "path", label: "Path" },
  { key: "query", label: "Query" },
  { key: "method", label: "Method" },
  { key: "protocol", label: "Protocol" },
  { key: "country_code", label: "Country code" },
  { key: "country_name", label: "Country" },
  { key: "city_name", label: "City" },
  { key: "ip_address", label: "IP" },
  { key: "user_agent", label: "User agent" },
  { key: "referer", label: "Referer" },
  { key: "content_type", label: "Content type" },
  { key: "accept_language", label: "Accept language" },
  { key: "x_request_id", label: "X-Request-Id" },
  { key: "is_tor", label: "TOR" },
];

type PipelineFilter = "all" | "waf" | "jail";
type ActionFilter = "all" | "allow" | "deny";

interface SelectOption {
  value: string;
  label: string;
}

interface State {
  wafTemplates: RuleTemplate[];
  jailTemplates: RuleTemplate[];
  loading: boolean;
  loadError: string | null;
  applying: string | null;
  search: string;
  pipelineFilter: PipelineFilter;
  severityFilter: string;
  categoryFilter: string;
  actionFilter: ActionFilter;
  mustHaveOnly: boolean;
  // Rule template apply modal
  modalVisible: boolean;
  selectedTemplate: RuleTemplate | null;
  fqdn: string;
  ipAddress: string;
  ruleName: string;
  ruleDescription: string;
  ruleWeight: number;
  ruleActive: boolean;
}

export default class TemplatesPage extends React.Component<
  Record<string, never>,
  State
> {
  constructor(props: Record<string, never>) {
    super(props);
    this.state = {
      wafTemplates: [],
      jailTemplates: [],
      loading: false,
      loadError: null,
      applying: null,
      search: "",
      pipelineFilter: "all",
      severityFilter: "all",
      categoryFilter: "all",
      actionFilter: "all",
      mustHaveOnly: false,
      modalVisible: false,
      selectedTemplate: null,
      fqdn: "",
      ipAddress: "",
      ruleName: "",
      ruleDescription: "",
      ruleWeight: 100,
      ruleActive: true,
    };
  }

  componentDidMount = async () => {
    this.setState({ loading: true });
    await this.loadTemplates();
    this.setState({ loading: false });
  };

  private loadTemplates = async () => {
    // `loadData` nunca rechaza: en error devuelve `{ status, message }` sin
    // `data`, así que aquí se distingue el fallo de un catálogo legítimamente
    // vacío y se hace visible al usuario en vez de descartarlo en silencio.
    const response = await loadData<TemplatesResponse>("templates", new Map());
    if (response.status === 200 && response.data) {
      this.setState({
        wafTemplates: response.data.waf,
        jailTemplates: response.data.jail,
        loadError: null,
      });
      return;
    }
    const msg =
      response.message || "Failed to load templates. Please try again.";
    message.error(msg);
    this.setState({ loadError: msg });
  };

  // --- Rule Template apply modal ---

  private openApplyModal = (template: RuleTemplate) => {
    this.setState({
      modalVisible: true,
      selectedTemplate: template,
      fqdn: "",
      ipAddress: "",
      ruleName: template.name,
      ruleDescription: template.description,
      ruleWeight: template.weight ?? 100,
    });
  };

  private closeModal = () => {
    this.setState({
      modalVisible: false,
      selectedTemplate: null,
      fqdn: "",
      ipAddress: "",
    });
  };

  private confirmApply = async () => {
    const template = this.state.selectedTemplate;
    if (!template) return;

    if (template.requires_fqdn && !this.state.fqdn.trim()) {
      message.error(
        `This template requires an FQDN (e.g. ${template.name.toLowerCase().replace(/[^a-z0-9]/g, "-")}.example.com)`,
      );
      return;
    }

    this.setState({ applying: template.name, modalVisible: false });
    try {
      const body: RuleCreatePayload = {
        name: this.state.ruleName,
        description: this.state.ruleDescription,
        mode: template.pipeline === "jail" ? "log_only" : "enforce",
        weight: this.state.ruleWeight,
        allow: template.allow,
        pipeline: template.pipeline,
        // All filter fields from template:
        path: template.path,
        query: template.query,
        country_code: template.country_code,
        ip_address: this.state.ipAddress || template.ip_address || null,
        protocol: template.protocol,
        city_name: template.city_name,
        country_name: template.country_name,
        user_agent: template.user_agent,
        method: template.method,
        referer: template.referer,
        content_type: template.content_type,
        accept_language: template.accept_language,
        x_request_id: template.x_request_id,
        is_tor: template.is_tor ?? null,
        fqdn: this.state.fqdn || null,
        active: this.state.ruleActive,
      };

      // If jail template has a rate_limit_profile_id, include it
      if (template.pipeline === "jail" && template.rate_limit_profile_id) {
        body.rate_limit_profile_id = template.rate_limit_profile_id;
      }

      const token = localStorage.getItem("token");
      const response = await fetch(`${BASE_URL}/api/v1/rules`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: JSON.stringify(body),
      });
      if (response.ok) {
        message.success(`Rule "${this.state.ruleName}" created successfully`);
      } else {
        if (response.status === 401) {
          window.location.href = "/login";
          return;
        }
        const err = await response.json();
        message.error(
          `Failed to create rule: ${err.message || response.statusText}`,
        );
      }
    } catch (e) {
      message.error(`Error: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      this.setState({ applying: null });
    }
  };

  // --- Listing: filtering, ordering and rendering ---

  /**
   * Catálogo completo (WAF + JAIL) filtrado por los filtros activos y el
   * buscador, y ordenado con las `must_have` primero y el resto por peso.
   */
  private visibleTemplates = (): RuleTemplate[] => {
    const search = this.state.search.trim().toLowerCase();
    const {
      pipelineFilter,
      severityFilter,
      categoryFilter,
      actionFilter,
      mustHaveOnly,
    } = this.state;

    const filtered = [...this.state.wafTemplates, ...this.state.jailTemplates].filter(
      (t) => {
        if (
          search &&
          !t.name.toLowerCase().includes(search) &&
          !t.description.toLowerCase().includes(search)
        ) {
          return false;
        }
        if (pipelineFilter !== "all" && t.pipeline !== pipelineFilter) {
          return false;
        }
        if (severityFilter !== "all" && t.severity !== severityFilter) {
          return false;
        }
        if (categoryFilter !== "all" && t.category !== categoryFilter) {
          return false;
        }
        if (actionFilter !== "all") {
          const action = t.allow ? "allow" : "deny";
          if (action !== actionFilter) return false;
        }
        if (mustHaveOnly && !t.must_have) return false;
        return true;
      },
    );

    return filtered.sort((a, b) => {
      if (a.must_have !== b.must_have) return a.must_have ? -1 : 1;
      return a.weight - b.weight;
    });
  };

  private categoryOptions = (): SelectOption[] => {
    const categories = [
      ...new Set(
        [...this.state.wafTemplates, ...this.state.jailTemplates].map(
          (t) => t.category,
        ),
      ),
    ].sort((a, b) => a.localeCompare(b));
    return [
      { value: "all", label: "All" },
      ...categories.map((category) => ({ value: category, label: category })),
    ];
  };

  private severityOptions = (): SelectOption[] => [
    { value: "all", label: "All" },
    { value: "critico", label: "critico" },
    { value: "alto", label: "alto" },
    { value: "medio", label: "medio" },
    { value: "bajo", label: "bajo" },
  ];

  private pipelineOptions: SelectOption[] = [
    { value: "all", label: "All" },
    { value: "waf", label: "WAF" },
    { value: "jail", label: "Jail" },
  ];

  private actionOptions: SelectOption[] = [
    { value: "all", label: "All" },
    { value: "allow", label: "Allow" },
    { value: "deny", label: "Deny" },
  ];

  private columns = (): TableColumnsType<RuleTemplate> => [
    {
      title: "Name",
      key: "name",
      render: (_, t) => (
        <Flex gap={4} align="center">
          <Text strong style={{ fontSize: 14 }}>
            {t.name}
          </Text>
          {t.must_have && (
            <Tag color="red" style={{ fontSize: 11, fontWeight: "bold" }}>
              MUST HAVE
            </Tag>
          )}
        </Flex>
      ),
    },
    {
      title: "Pipeline",
      key: "pipeline",
      render: (_, t) =>
        t.pipeline === "jail" ? (
          <Tag icon={<LockOutlined />} color="orange">
            Jail
          </Tag>
        ) : (
          <Tag color="blue">WAF</Tag>
        ),
    },
    {
      title: "Category",
      key: "category",
      render: (_, t) => <Tag>{CATEGORY_LABELS[t.category] || t.category}</Tag>,
    },
    {
      title: "Severity",
      key: "severity",
      render: (_, t) => (
        <Tag color={SEVERITY_COLORS[t.severity] || "default"}>
          {SEVERITY_LABELS[t.severity] || t.severity}
        </Tag>
      ),
    },
    {
      title: "Action",
      key: "action",
      render: (_, t) =>
        t.pipeline === "jail" ? (
          <Tag color="purple">Rate limit</Tag>
        ) : (
          <Tag
            icon={t.allow ? <CheckCircleOutlined /> : <CloseCircleOutlined />}
            color={t.allow ? "success" : "error"}
          >
            {t.allow ? "Allow" : "Deny"}
          </Tag>
        ),
    },
    {
      title: "Profile",
      key: "profile",
      render: (_, t) =>
        t.pipeline === "jail" && t.rate_limit_profile_name ? (
          <Tag color="orange" style={{ fontSize: 11 }}>
            {t.rate_limit_profile_name}
          </Tag>
        ) : (
          <Text type="secondary">—</Text>
        ),
    },
    {
      title: "Actions",
      key: "apply",
      width: 140,
      render: (_, t) => (
        <Button
          type="primary"
          aria-label={`Apply template ${t.name}`}
          icon={<ThunderboltOutlined />}
          loading={this.state.applying === t.name}
          onClick={() => this.openApplyModal(t)}
        >
          Apply
        </Button>
      ),
    },
  ];

  private renderFilterDetails = (t: RuleTemplate): React.ReactNode => {
    const details = FILTER_DETAIL_FIELDS.flatMap(({ key, label }) => {
      const value = t[key];
      if (value === null || value === undefined) return [];
      const display = typeof value === "boolean" ? (value ? "yes" : "no") : value;
      return [
        <Tag key={key} color="blue">
          {label}: {display}
        </Tag>,
      ];
    });

    if (details.length === 0) {
      return (
        <Text type="secondary">
          No filters — this template matches all traffic.
        </Text>
      );
    }

    return (
      <Flex wrap gap={4} align="center">
        {details}
      </Flex>
    );
  };

  render = () => {
    const selected = this.state.selectedTemplate;

    if (this.state.loading) {
      return (
        <Flex justify="center" align="center" style={{ minHeight: 400 }}>
          <Spin size="large" />
        </Flex>
      );
    }

    const templates = this.visibleTemplates();

    return (
      <Flex vertical gap="middle" style={{ padding: 24 }}>
        {/* El buscador ocupa su propia línea, a ancho completo, para no competir
            por el espacio con los filtros. */}
        <Input
          prefix={<SearchOutlined />}
          aria-label="Search templates"
          placeholder="Search templates..."
          style={{ width: "100%" }}
          value={this.state.search}
          onChange={(e) => this.setState({ search: e.target.value })}
          allowClear
        />

        {/* Rejilla de anchos fijos: cada filtro mide lo mismo sea cual sea el
            ancho de la ventana. `Row` envuelve por defecto. */}
        <Row gutter={[12, 12]} align="middle">
          <Col flex="0 0 160px">
            <Select
              aria-label="Pipeline"
              style={{ width: "100%" }}
              placeholder="All"
              value={this.state.pipelineFilter === "all" ? undefined : this.state.pipelineFilter}
              options={this.pipelineOptions}
              onChange={(value: PipelineFilter) =>
                this.setState({ pipelineFilter: value })
              }
            />
          </Col>
          <Col flex="0 0 160px">
            <Select
              aria-label="Severity"
              style={{ width: "100%" }}
              placeholder="All"
              value={this.state.severityFilter === "all" ? undefined : this.state.severityFilter}
              options={this.severityOptions()}
              onChange={(value: string) =>
                this.setState({ severityFilter: value })
              }
            />
          </Col>
          <Col flex="0 0 240px">
            <Select
              aria-label="Category"
              style={{ width: "100%" }}
              placeholder="All"
              value={this.state.categoryFilter === "all" ? undefined : this.state.categoryFilter}
              options={this.categoryOptions()}
              onChange={(value: string) =>
                this.setState({ categoryFilter: value })
              }
            />
          </Col>
          <Col flex="0 0 160px">
            <Select
              aria-label="Action"
              style={{ width: "100%" }}
              placeholder="All"
              value={this.state.actionFilter === "all" ? undefined : this.state.actionFilter}
              options={this.actionOptions}
              onChange={(value: ActionFilter) =>
                this.setState({ actionFilter: value })
              }
            />
          </Col>
          <Col flex="0 0 auto">
            <Flex gap={4} align="center">
              <Switch
                aria-label="Must have"
                checked={this.state.mustHaveOnly}
                onChange={(checked) => this.setState({ mustHaveOnly: checked })}
              />
              <Text>Must have</Text>
            </Flex>
          </Col>
        </Row>

        <Text type="secondary">
          Browse the rule templates. Expand a row to see the full filters a
          template carries, or apply it to create a new rule.
        </Text>

        {templates.length === 0 ? (
          <Empty
            description={this.state.loadError ?? "No templates found"}
            style={{ padding: 48 }}
          />
        ) : (
          <Table<RuleTemplate>
            rowKey={(t) => t.name}
            columns={this.columns()}
            dataSource={templates}
            pagination={false}
            expandable={{ expandedRowRender: this.renderFilterDetails }}
          />
        )}

        {/* Rule Template Apply modal */}
        <Modal
          title={`Apply Template: ${selected?.name || ""}`}
          open={this.state.modalVisible}
          onOk={this.confirmApply}
          onCancel={this.closeModal}
          okText="Create Rule"
          cancelText="Cancel"
          width={600}
        >
          <Flex vertical gap="middle">
            {/* ── Preview: what will be created ── */}
            <div
              style={{
                background: "var(--color-bg-layout)",
                borderRadius: 8,
                border: "1px solid var(--color-border)",
                padding: 16,
              }}
            >
              <Text
                strong
                style={{ fontSize: 15, marginBottom: 12, display: "block" }}
              >
                ⚡ Rule to be created
              </Text>

              {/* Pipeline + Action row */}
              <Flex wrap gap="small" style={{ marginBottom: 12 }}>
                {selected?.pipeline === "jail" ? (
                  <Tag
                    icon={<LockOutlined />}
                    color="orange"
                    style={{ fontSize: 13, padding: "2px 8px" }}
                  >
                    Pipeline: Jail
                  </Tag>
                ) : (
                  <Tag
                    color="blue"
                    style={{ fontSize: 13, padding: "2px 8px" }}
                  >
                    Pipeline: WAF
                  </Tag>
                )}
                {selected?.pipeline !== "jail" && (
                  <Tag
                    icon={
                      selected?.allow ? (
                        <CheckCircleOutlined />
                      ) : (
                        <CloseCircleOutlined />
                      )
                    }
                    color={selected?.allow ? "success" : "error"}
                    style={{ fontSize: 13, padding: "2px 8px" }}
                  >
                    Action: {selected?.allow ? "Allow" : "Deny"}
                  </Tag>
                )}
                <Tag style={{ fontSize: 13, padding: "2px 8px" }}>
                  Weight: {this.state.ruleWeight}
                </Tag>
                <Tag
                  color={this.state.ruleActive ? "green" : "default"}
                  style={{ fontSize: 13, padding: "2px 8px" }}
                >
                  Active: {this.state.ruleActive ? "Yes" : "No"}
                </Tag>
                {selected?.pipeline !== "jail" && (
                  <Tag
                    color="purple"
                    style={{ fontSize: 13, padding: "2px 8px" }}
                  >
                    Mode: Enforce
                  </Tag>
                )}
                {selected?.pipeline === "jail" && (
                  <Tag
                    color="purple"
                    style={{ fontSize: 13, padding: "2px 8px" }}
                  >
                    Mode: Log Only
                  </Tag>
                )}
              </Flex>

              {/* Filters section */}
              <Text
                strong
                style={{ fontSize: 13, marginBottom: 8, display: "block" }}
              >
                🔍 Filters
              </Text>
              <Flex wrap gap="small" style={{ marginBottom: 12 }}>
                {selected?.path ? (
                  <Tag color="blue" style={{ fontSize: 12 }}>
                    Path: {selected.path}
                  </Tag>
                ) : (
                  <Tag style={{ fontSize: 12 }}>Path: —</Tag>
                )}
                {selected?.query ? (
                  <Tag color="purple" style={{ fontSize: 12 }}>
                    Query: {selected.query}
                  </Tag>
                ) : (
                  <Tag style={{ fontSize: 12 }}>Query: —</Tag>
                )}
                {selected?.country_code ? (
                  <Tag color="cyan" style={{ fontSize: 12 }}>
                    Country: {selected.country_code}
                  </Tag>
                ) : (
                  <Tag style={{ fontSize: 12 }}>Country: —</Tag>
                )}
                {selected?.user_agent && (
                  <Tag color="volcano" style={{ fontSize: 12 }}>
                    UA: {selected.user_agent}
                  </Tag>
                )}
                {selected?.method && (
                  <Tag color="magenta" style={{ fontSize: 12 }}>
                    Method: {selected.method}
                  </Tag>
                )}
                {selected?.referer && (
                  <Tag color="purple" style={{ fontSize: 12 }}>
                    Referer: {selected.referer}
                  </Tag>
                )}
                {selected?.content_type && (
                  <Tag color="cyan" style={{ fontSize: 12 }}>
                    Content-Type: {selected.content_type}
                  </Tag>
                )}
                {selected?.city_name && (
                  <Tag color="geekblue" style={{ fontSize: 12 }}>
                    City: {selected.city_name}
                  </Tag>
                )}
                {selected?.country_name && (
                  <Tag color="green" style={{ fontSize: 12 }}>
                    Country: {selected.country_name}
                  </Tag>
                )}
                {selected?.ip_address && (
                  <Tag color="lime" style={{ fontSize: 12 }}>
                    IP: {selected.ip_address}
                  </Tag>
                )}
                {selected?.protocol && (
                  <Tag color="gold" style={{ fontSize: 12 }}>
                    Protocol: {selected.protocol}
                  </Tag>
                )}
                {selected?.accept_language && (
                  <Tag color="orange" style={{ fontSize: 12 }}>
                    Accept-Lang: {selected.accept_language}
                  </Tag>
                )}
                {selected?.x_request_id && (
                  <Tag style={{ fontSize: 12 }}>
                    X-Request-ID: {selected.x_request_id}
                  </Tag>
                )}
              </Flex>

              {/* Scope section */}
              <Text
                strong
                style={{ fontSize: 13, marginBottom: 8, display: "block" }}
              >
                🎯 Scope
              </Text>
              <Flex wrap gap="small">
                <Tag style={{ fontSize: 12 }}>
                  FQDN: {this.state.fqdn.trim() || "(all)"}
                </Tag>
                <Tag style={{ fontSize: 12 }}>
                  IP: {this.state.ipAddress.trim() || "(all)"}
                </Tag>
              </Flex>

              {/* Rate limit profile for jail */}
              {selected?.pipeline === "jail" &&
                selected?.rate_limit_profile_name && (
                  <>
                    <div
                      style={{
                        borderTop: "1px solid var(--color-border)",
                        margin: "12px 0",
                      }}
                    />
                    <Text
                      strong
                      style={{
                        fontSize: 13,
                        marginBottom: 8,
                        display: "block",
                      }}
                    >
                      ⏱ Rate Limit Profile
                    </Text>
                    <Tag
                      icon={<LockOutlined />}
                      color="orange"
                      style={{ fontSize: 12 }}
                    >
                      {selected.rate_limit_profile_name}
                    </Tag>
                  </>
                )}
            </div>

            {/* ── Editable fields ── */}
            <Flex vertical gap="small">
              <Text strong>Rule settings</Text>
              <Input
                placeholder="Rule name"
                value={this.state.ruleName}
                onChange={(e) => this.setState({ ruleName: e.target.value })}
                addonBefore="Name"
                allowClear
              />
              <Input
                placeholder="Rule description"
                value={this.state.ruleDescription}
                onChange={(e) =>
                  this.setState({ ruleDescription: e.target.value })
                }
                addonBefore="Description"
                allowClear
              />
              <Flex align="center" gap="small">
                <Text style={{ width: 100, flexShrink: 0 }}>Weight</Text>
                <InputNumber
                  style={{ width: "100%" }}
                  value={this.state.ruleWeight}
                  min={1}
                  max={99999}
                  onChange={(value) =>
                    this.setState({ ruleWeight: value ?? 100 })
                  }
                />
              </Flex>
              <Flex align="center" gap="small">
                <Text style={{ width: 100, flexShrink: 0 }}>Active</Text>
                <Switch
                  checked={this.state.ruleActive}
                  onChange={(checked) => this.setState({ ruleActive: checked })}
                />
              </Flex>
            </Flex>

            {/* ── Scope inputs ── */}
            <Flex vertical gap="small">
              <Text strong>Configure scope</Text>
              {selected?.requires_fqdn ? (
                <>
                  <Text
                    type="warning"
                    style={{ color: "#fa8c16", fontSize: 12 }}
                  >
                    This template is for a specific service and requires an FQDN
                    to avoid matching unintended traffic.
                  </Text>
                  <Input
                    placeholder="FQDN (e.g. service.example.com)"
                    value={this.state.fqdn}
                    onChange={(e) => this.setState({ fqdn: e.target.value })}
                    status={this.state.fqdn.trim() ? undefined : "error"}
                    allowClear
                  />
                </>
              ) : (
                <>
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    This is a general template. Leave empty to apply to all
                    traffic, or specify a specific FQDN to scope it.
                  </Text>
                  <Input
                    placeholder="FQDN (optional)"
                    value={this.state.fqdn}
                    onChange={(e) => this.setState({ fqdn: e.target.value })}
                    allowClear
                  />
                </>
              )}
              <Input
                placeholder="IP Address (optional)"
                value={this.state.ipAddress}
                onChange={(e) => this.setState({ ipAddress: e.target.value })}
                allowClear
              />
            </Flex>
          </Flex>
        </Modal>
      </Flex>
    );
  };
}
