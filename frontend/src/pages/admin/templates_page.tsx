import React from "react";
import { Card, Collapse, Tag, Button, Typography, Flex, message, Input, Modal } from 'antd';
import { CheckCircleOutlined, CloseCircleOutlined, ThunderboltOutlined, SearchOutlined } from '@ant-design/icons';
import type Item from "@/models/template";
import { loadData } from '@/common/utils';
import { BASE_URL, VERSION } from '@/constants';

const { Text, Title } = Typography;

const SEVERITY_COLORS: Record<string, string> = {
    "🔥 Crítico": "red",
    "🔴 Alto": "orange",
    "🟡 Medio": "gold",
    "🟢 Bajo": "green",
};

interface State {
    templates: Item[];
    loading: boolean;
    applying: string | null;
    search: string;
    // Modal state
    modalVisible: boolean;
    selectedTemplate: Item | null;
    fqdn: string;
    ipAddress: string;
}

export default class TemplatesPage extends React.Component<{}, State> {
    constructor(props: {}) {
        super(props);
        this.state = {
            templates: [],
            loading: false,
            applying: null,
            search: "",
            modalVisible: false,
            selectedTemplate: null,
            fqdn: "",
            ipAddress: "",
        };
    }

    componentDidMount = async () => {
        this.setState({ loading: true });
        const response = await loadData<Item[]>("templates", new Map());
        if (response.status === 200 && response.data) {
            this.setState({ templates: response.data, loading: false });
        } else {
            this.setState({ loading: false });
        }
    }

    private openApplyModal = (template: Item) => {
        this.setState({
            modalVisible: true,
            selectedTemplate: template,
            fqdn: "",
            ipAddress: "",
        });
    }

    private closeModal = () => {
        this.setState({
            modalVisible: false,
            selectedTemplate: null,
            fqdn: "",
            ipAddress: "",
        });
    }

    private confirmApply = async () => {
        const template = this.state.selectedTemplate;
        if (!template) return;

        // Validate FQDN if required
        if (template.requires_fqdn && !this.state.fqdn.trim()) {
            message.error(`This template requires an FQDN (e.g. ${template.name.toLowerCase().replace(/[^a-z0-9]/g, '-')}.example.com)`);
            this.setState({ modalVisible: true });
            return;
        }

        this.setState({ applying: template.name, modalVisible: false });
        try {
            const body: any = {
                weight: 100,
                allow: template.allow,
                store: template.store,
                path: template.path,
                query: template.query,
                country_code: template.country_code,
                fqdn: this.state.fqdn || null,
                ip_address: this.state.ipAddress || null,
                rate_limit_enabled: template.rate_limit_enabled,
                max_retry: template.max_retry,
                find_time_seconds: template.find_time_seconds,
                ban_time_seconds: template.ban_time_seconds,
                bantime_increment: template.bantime_increment,
                bantime_multipliers: template.bantime_multipliers,
                bantime_maxtime_seconds: template.bantime_maxtime_seconds,
                ban_count_decay_days: template.ban_count_decay_days,
                active: true,
            };
            const response = await fetch(`${BASE_URL}/api/${VERSION}/rules`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(body),
            });
            if (response.ok) {
                message.success(`Rule "${template.name}" created successfully`);
            } else {
                const err = await response.json();
                message.error(`Failed to create rule: ${err.message || response.statusText}`);
            }
        } catch (e: any) {
            message.error(`Error: ${e.message}`);
        } finally {
            this.setState({ applying: null });
        }
    }

    private groupedByCategory = (): Map<string, Item[]> => {
        const groups = new Map<string, Item[]>();
        const searchLower = this.state.search.toLowerCase();
        const filtered = this.state.templates.filter(t =>
            t.name.toLowerCase().includes(searchLower) ||
            t.description.toLowerCase().includes(searchLower) ||
            t.category.toLowerCase().includes(searchLower)
        );
        for (const template of filtered) {
            const cat = template.category;
            if (!groups.has(cat)) {
                groups.set(cat, []);
            }
            groups.get(cat)!.push(template);
        }
        return groups;
    }

    private renderRateLimitInfo = (template: Item) => {
        if (!template.rate_limit_enabled) {
            return <Text type="secondary">Rate limiting disabled</Text>;
        }
        return (
            <Flex wrap gap="small" align="center">
                <Tag>Max retry: {template.max_retry}</Tag>
                <Tag>Find time: {template.find_time_seconds}s</Tag>
                <Tag>Ban time: {template.ban_time_seconds}s</Tag>
                {template.bantime_increment && (
                    <Tag color="orange">Escalation: {template.bantime_multipliers.join("×, ")}×</Tag>
                )}
                <Tag>Max ban: {template.bantime_maxtime_seconds >= 86400
                    ? `${Math.floor(template.bantime_maxtime_seconds / 86400)}d`
                    : `${template.bantime_maxtime_seconds}s`}</Tag>
            </Flex>
        );
    }

    render = () => {
        const groups = this.groupedByCategory();
        const categoryLabels: Record<string, string> = {
            "wordpress": "WordPress",
            "joomla": "Joomla",
            "drupal": "Drupal",
            "laravel": "Laravel",
            "paneles": "Paneles de administración",
            "api": "API",
            "servidores": "Servidores de aplicaciones",
            "seguridad": "Seguridad general",
            "bots": "Bots y scrapers",
            "geo": "Geo",
            "cms": "CMS",
            "infra": "Infraestructura",
            "probes": "Probes de seguridad",
            "webmail": "Webmail",
        };

        const collapseItems = Array.from(groups.entries()).map(([category, templates]) => ({
            key: category,
            label: (
                <Flex justify="space-between" style={{ width: '100%' }}>
                    <Text strong style={{ fontSize: 16 }}>
                        {categoryLabels[category] || category}
                    </Text>
                    <Tag>{templates.length} templates</Tag>
                </Flex>
            ),
            children: (
                <Flex wrap gap="small" style={{ width: '100%' }}>
                    {templates.map(t => (
                        <Card
                            key={t.name}
                            size="small"
                            variant="outlined"
                            style={{ width: '48%', minWidth: 400, marginBottom: 8 }}
                            actions={[
                                <Button
                                    type="primary"
                                    icon={<ThunderboltOutlined />}
                                    loading={this.state.applying === t.name}
                                    onClick={() => this.openApplyModal(t)}
                                >
                                    Apply
                                </Button>
                            ]}
                        >
                            <Flex vertical gap="small">
                                <Flex justify="space-between" align="center">
                                    <Text strong>{t.name}</Text>
                                    <Tag color={SEVERITY_COLORS[t.severity] || "default"}>
                                        {t.severity}
                                    </Tag>
                                </Flex>
                                <Text type="secondary">{t.description}</Text>
                                <Flex wrap gap="small" align="center">
                                    {t.allow
                                        ? <Tag icon={<CheckCircleOutlined />} color="success">Allow</Tag>
                                        : <Tag icon={<CloseCircleOutlined />} color="error">Deny</Tag>
                                    }
                                    {t.path && <Tag color="blue">Path: {t.path}</Tag>}
                                    {t.query && <Tag color="purple">Query: {t.query}</Tag>}
                                    {t.country_code && <Tag color="cyan">Geo: {t.country_code}</Tag>}
                                </Flex>
                                {this.renderRateLimitInfo(t)}
                            </Flex>
                        </Card>
                    ))}
                </Flex>
            ),
        }));

        const selected = this.state.selectedTemplate;

        return (
            <Flex vertical gap="middle" style={{ padding: 24 }}>
                <Flex justify="space-between" align="center">
                    <Title level={3} style={{ margin: 0 }}>Rule Templates</Title>
                    <Input
                        prefix={<SearchOutlined />}
                        placeholder="Search templates..."
                        style={{ width: 300 }}
                        value={this.state.search}
                        onChange={e => this.setState({ search: e.target.value })}
                        allowClear
                    />
                </Flex>
                <Text type="secondary">
                    Select a template to apply as a new rule. When applying, you can optionally scope it to a specific FQDN or IP address.
                    Templates are preconfigured with recommended settings for each service.
                </Text>
                <Collapse
                    items={collapseItems}
                    defaultActiveKey={Array.from(groups.keys()).slice(0, 2)}
                />

                {/* Apply modal */}
                <Modal
                    title={`Apply Template: ${selected?.name || ""}`}
                    open={this.state.modalVisible}
                    onOk={this.confirmApply}
                    onCancel={this.closeModal}
                    okText="Create Rule"
                    cancelText="Cancel"
                >
                    <Flex vertical gap="middle">
                        <Text type="secondary">{selected?.description}</Text>
                        <Flex vertical gap="small">
                            <Text strong>Scope</Text>
                            {selected?.requires_fqdn ? (
                                <>
                                    <Text type="warning" style={{ color: '#fa8c16' }}>
                                        This template is for a specific service and requires an FQDN to avoid
                                        matching unintended traffic.
                                    </Text>
                                    <Input
                                        placeholder="FQDN (e.g. service.example.com)"
                                        value={this.state.fqdn}
                                        onChange={e => this.setState({ fqdn: e.target.value })}
                                        status={this.state.fqdn.trim() ? undefined : 'error'}
                                        allowClear
                                    />
                                </>
                            ) : (
                                <>
                                    <Text type="secondary">
                                        This is a general security template. Leave empty to apply to all traffic,
                                        or specify a specific FQDN to scope it.
                                    </Text>
                                    <Input
                                        placeholder="FQDN (optional — applies to all traffic if empty)"
                                        value={this.state.fqdn}
                                        onChange={e => this.setState({ fqdn: e.target.value })}
                                        allowClear
                                    />
                                </>
                            )}
                            <Input
                                placeholder="IP Address (optional)"
                                value={this.state.ipAddress}
                                onChange={e => this.setState({ ipAddress: e.target.value })}
                                allowClear
                            />
                        </Flex>
                        <Flex wrap gap="small">
                            <Tag>{selected?.allow ? "Allow" : "Deny"}</Tag>
                            {selected?.path && <Tag color="blue">Path: {selected.path}</Tag>}
                            {selected?.query && <Tag color="purple">Query: {selected.query}</Tag>}
                            {selected?.rate_limit_enabled && <Tag color="orange">Rate limited</Tag>}
                        </Flex>
                    </Flex>
                </Modal>
            </Flex>
        );
    }
}