import React from "react";
import { Card, Collapse, Tag, Button, Typography, Flex, message, Input, Modal, Tabs, Spin } from 'antd';
import type { TabsProps } from 'antd';
import { CheckCircleOutlined, CloseCircleOutlined, ThunderboltOutlined, SearchOutlined } from '@ant-design/icons';
import type { RuleTemplate, RateLimitProfileTemplate } from "@/models/template";
import { loadData } from '@/common/utils';
import { BASE_URL } from '@/constants';

const { Text, Title } = Typography;

const SEVERITY_COLORS: Record<string, string> = {
    "🔥 Crítico": "red",
    "🔴 Alto": "orange",
    "🟡 Medio": "gold",
    "🟢 Bajo": "green",
};

interface State {
    ruleTemplates: RuleTemplate[];
    profileTemplates: RateLimitProfileTemplate[];
    loading: boolean;
    applying: string | null;
    search: string;
    // Rule template modal state
    modalVisible: boolean;
    selectedTemplate: RuleTemplate | null;
    fqdn: string;
    ipAddress: string;
    // Profile template modal state
    profileModalVisible: boolean;
    selectedProfileTemplate: RateLimitProfileTemplate | null;
}

export default class TemplatesPage extends React.Component<{}, State> {
    constructor(props: {}) {
        super(props);
        this.state = {
            ruleTemplates: [],
            profileTemplates: [],
            loading: false,
            applying: null,
            search: "",
            modalVisible: false,
            selectedTemplate: null,
            fqdn: "",
            ipAddress: "",
            profileModalVisible: false,
            selectedProfileTemplate: null,
        };
    }

    componentDidMount = async () => {
        this.setState({ loading: true });
        await Promise.all([
            this.loadRuleTemplates(),
            this.loadProfileTemplates(),
        ]);
        this.setState({ loading: false });
    }

    private loadRuleTemplates = async () => {
        const response = await loadData<RuleTemplate[]>("templates/rules", new Map());
        if (response.status === 200 && response.data) {
            this.setState({ ruleTemplates: response.data });
        }
    }

    private loadProfileTemplates = async () => {
        const response = await loadData<RateLimitProfileTemplate[]>("templates/rate-limit-profiles", new Map());
        if (response.status === 200 && response.data) {
            this.setState({ profileTemplates: response.data });
        }
    }

    // --- Rule Template modal handlers ---

    private openApplyModal = (template: RuleTemplate) => {
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
                name: template.name,
                description: template.description,
                mode: "enforce",
                weight: 100,
                allow: template.allow,
                store: template.store,
                path: template.path,
                query: template.query,
                country_code: template.country_code,
                fqdn: this.state.fqdn || null,
                ip_address: this.state.ipAddress || null,
                active: true,
            };
            const token = localStorage.getItem("token");
            const response = await fetch(`${BASE_URL}/api/v1/rules`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    ...(token ? { 'Authorization': `Bearer ${token}` } : {}),
                },
                body: JSON.stringify(body),
            });
            if (response.ok) {
                message.success(`Rule "${template.name}" created successfully`);
                if (template.recommended_profile) {
                    message.info(`Recommended rate limit profile: "${template.recommended_profile}"`);
                }
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

    // --- Profile Template modal handlers ---

    private openProfileApplyModal = (template: RateLimitProfileTemplate) => {
        this.setState({
            profileModalVisible: true,
            selectedProfileTemplate: template,
        });
    }

    private closeProfileModal = () => {
        this.setState({
            profileModalVisible: false,
            selectedProfileTemplate: null,
        });
    }

    private confirmApplyProfile = async () => {
        const template = this.state.selectedProfileTemplate;
        if (!template) return;

        this.setState({ applying: template.name, profileModalVisible: false });
        try {
            const body: any = {
                name: template.name,
                description: template.description,
                max_retry: template.max_retry,
                find_time_seconds: template.find_time_seconds,
                ban_time_seconds: template.ban_time_seconds,
                bantime_increment: template.bantime_increment,
                bantime_multipliers: template.bantime_multipliers,
                bantime_maxtime_seconds: template.bantime_maxtime_seconds,
                ban_count_decay_days: template.ban_count_decay_days,
            };
            const token = localStorage.getItem("token");
            const response = await fetch(`${BASE_URL}/api/v1/rate-limit-profiles`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    ...(token ? { 'Authorization': `Bearer ${token}` } : {}),
                },
                body: JSON.stringify(body),
            });
            if (response.ok) {
                message.success(`Rate limit profile "${template.name}" created successfully`);
            } else {
                const err = await response.json();
                message.error(`Failed to create profile: ${err.message || response.statusText}`);
            }
        } catch (e: any) {
            message.error(`Error: ${e.message}`);
        } finally {
            this.setState({ applying: null });
        }
    }

    // --- Grouping and rendering ---

    private groupedByCategory = (): Map<string, RuleTemplate[]> => {
        const groups = new Map<string, RuleTemplate[]>();
        const searchLower = this.state.search.toLowerCase();
        const filtered = this.state.ruleTemplates.filter(t =>
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

    private renderRuleTemplateCard = (t: RuleTemplate) => (
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
                {t.recommended_profile && (
                    <Tag color="orange">Recommended profile: {t.recommended_profile}</Tag>
                )}
            </Flex>
        </Card>
    );

    private renderProfileTemplateCard = (t: RateLimitProfileTemplate) => (
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
                    onClick={() => this.openProfileApplyModal(t)}
                >
                    Apply
                </Button>
            ]}
        >
            <Flex vertical gap="small">
                <Text strong>{t.name}</Text>
                <Text type="secondary">{t.description}</Text>
                <Flex wrap gap="small" align="center">
                    <Tag>Max retry: {t.max_retry}</Tag>
                    <Tag>Find time: {t.find_time_seconds}s</Tag>
                    <Tag>Ban time: {t.ban_time_seconds}s</Tag>
                    {t.bantime_increment && (
                        <Tag color="orange">Escalation: {t.bantime_multipliers.join("×, ")}×</Tag>
                    )}
                    <Tag>Max ban: {t.bantime_maxtime_seconds >= 86400
                        ? `${Math.floor(t.bantime_maxtime_seconds / 86400)}d`
                        : `${t.bantime_maxtime_seconds}s`}</Tag>
                    <Tag>Decay: {t.ban_count_decay_days}d</Tag>
                </Flex>
            </Flex>
        </Card>
    );

    private renderRuleTemplatesSection = () => {
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
                    {templates.map(t => this.renderRuleTemplateCard(t))}
                </Flex>
            ),
        }));

        return (
            <Flex vertical gap="middle">
                <Flex justify="space-between" align="center">
                    <Title level={4} style={{ margin: 0 }}>Rule Templates</Title>
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
                {collapseItems.length > 0 ? (
                    <Collapse
                        items={collapseItems}
                        defaultActiveKey={Array.from(groups.keys()).slice(0, 2)}
                    />
                ) : (
                    <Text type="secondary" style={{ textAlign: 'center', padding: 24 }}>
                        No rule templates found matching your search.
                    </Text>
                )}
            </Flex>
        );
    }

    private renderProfileTemplatesSection = () => {
        const searchLower = this.state.search.toLowerCase();
        const filtered = this.state.profileTemplates.filter(t =>
            t.name.toLowerCase().includes(searchLower) ||
            t.description.toLowerCase().includes(searchLower)
        );

        return (
            <Flex vertical gap="middle">
                <Flex justify="space-between" align="center">
                    <Title level={4} style={{ margin: 0 }}>Rate Limit Profile Templates</Title>
                    <Input
                        prefix={<SearchOutlined />}
                        placeholder="Search profiles..."
                        style={{ width: 300 }}
                        value={this.state.search}
                        onChange={e => this.setState({ search: e.target.value })}
                        allowClear
                    />
                </Flex>
                <Text type="secondary">
                    Apply a rate limit profile template to create a new rate limit profile with preconfigured settings.
                    These profiles can then be assigned to rules.
                </Text>
                {filtered.length > 0 ? (
                    <Flex wrap gap="small" style={{ width: '100%' }}>
                        {filtered.map(t => this.renderProfileTemplateCard(t))}
                    </Flex>
                ) : (
                    <Text type="secondary" style={{ textAlign: 'center', padding: 24 }}>
                        No rate limit profile templates found matching your search.
                    </Text>
                )}
            </Flex>
        );
    }

    render = () => {
        const selected = this.state.selectedTemplate;
        const selectedProfile = this.state.selectedProfileTemplate;

        const tabItems: TabsProps['items'] = [
            {
                key: 'rule-templates',
                label: 'Rule Templates',
                children: this.renderRuleTemplatesSection(),
            },
            {
                key: 'profile-templates',
                label: 'Rate Limit Profiles',
                children: this.renderProfileTemplatesSection(),
            },
        ];

        if (this.state.loading) {
            return (
                <Flex justify="center" align="center" style={{ minHeight: 400 }}>
                    <Spin size="large" />
                </Flex>
            );
        }

        return (
            <Flex vertical gap="middle" style={{ padding: 24 }}>
                <Title level={3} style={{ margin: 0 }}>Templates</Title>
                <Tabs defaultActiveKey="rule-templates" items={tabItems} size="large" />

                {/* Rule Template Apply modal */}
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
                        {selected?.recommended_profile && (
                            <Tag color="orange">Recommended profile: {selected.recommended_profile}</Tag>
                        )}
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
                        </Flex>
                    </Flex>
                </Modal>

                {/* Profile Template Apply modal */}
                <Modal
                    title={`Apply Profile Template: ${selectedProfile?.name || ""}`}
                    open={this.state.profileModalVisible}
                    onOk={this.confirmApplyProfile}
                    onCancel={this.closeProfileModal}
                    okText="Create Profile"
                    cancelText="Cancel"
                >
                    <Flex vertical gap="middle">
                        <Text type="secondary">{selectedProfile?.description}</Text>
                        <Flex wrap gap="small">
                            <Tag>Max retry: {selectedProfile?.max_retry}</Tag>
                            <Tag>Find time: {selectedProfile?.find_time_seconds}s</Tag>
                            <Tag>Ban time: {selectedProfile?.ban_time_seconds}s</Tag>
                            {selectedProfile?.bantime_increment && (
                                <Tag color="orange">Escalation enabled</Tag>
                            )}
                            <Tag>Max ban: {selectedProfile?.bantime_maxtime_seconds}s</Tag>
                            <Tag>Decay: {selectedProfile?.ban_count_decay_days}d</Tag>
                        </Flex>
                        <Text>
                            This will create a new rate limit profile with the settings above.
                            You can then assign it to rules from the Rules page.
                        </Text>
                    </Flex>
                </Modal>
            </Flex>
        );
    }
}