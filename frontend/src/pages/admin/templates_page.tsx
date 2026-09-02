import React from "react";
import { Card, Collapse, Tag, Button, Typography, Flex, message, Input, InputNumber, Modal, Tabs, Spin, Empty, Switch } from 'antd';
import type { TabsProps } from 'antd';
import { CheckCircleOutlined, CloseCircleOutlined, ThunderboltOutlined, SearchOutlined, LockOutlined, GlobalOutlined, ApiOutlined, WarningOutlined, RobotOutlined, CloudServerOutlined, MailOutlined, AppstoreOutlined, SafetyOutlined, CodeOutlined } from '@ant-design/icons';
import type { RuleTemplate, RateLimitProfileTemplate, TemplatesResponse } from "@/models/template";
import { loadData } from '@/common/utils';
import { BASE_URL } from '@/constants';

const { Text, Title } = Typography;

const SEVERITY_COLORS: Record<string, string> = {
    "🔥 Crítico": "red",
    "🔴 Alto": "orange",
    "🟡 Medio": "gold",
    "🟢 Bajo": "green",
};

const CATEGORY_ICONS: Record<string, React.ReactNode> = {
    "wordpress": <AppstoreOutlined />,
    "joomla": <AppstoreOutlined />,
    "drupal": <AppstoreOutlined />,
    "laravel": <CodeOutlined />,
    "paneles": <LockOutlined />,
    "api": <ApiOutlined />,
    "servidores": <CloudServerOutlined />,
    "seguridad": <SafetyOutlined />,
    "bots": <RobotOutlined />,
    "geo": <GlobalOutlined />,
    "cms": <AppstoreOutlined />,
    "infra": <CloudServerOutlined />,
    "probes": <WarningOutlined />,
    "webmail": <MailOutlined />,
};

const CATEGORY_LABELS: Record<string, string> = {
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

interface State {
    wafTemplates: RuleTemplate[];
    jailTemplates: RuleTemplate[];
    profiles: RateLimitProfileTemplate[];
    loading: boolean;
    applying: string | null;
    search: string;
    activeTab: string;
    // Rule template apply modal
    modalVisible: boolean;
    selectedTemplate: RuleTemplate | null;
    fqdn: string;
    ipAddress: string;
    ruleName: string;
    ruleDescription: string;
    ruleWeight: number;
    ruleActive: boolean;
    ruleStore: boolean;
    // Profile template apply modal
    profileModalVisible: boolean;
    selectedProfile: RateLimitProfileTemplate | null;
}

export default class TemplatesPage extends React.Component<{}, State> {
    constructor(props: {}) {
        super(props);
        this.state = {
            wafTemplates: [],
            jailTemplates: [],
            profiles: [],
            loading: false,
            applying: null,
            search: "",
            activeTab: "waf",
            modalVisible: false,
            selectedTemplate: null,
            fqdn: "",
            ipAddress: "",
            ruleName: "",
            ruleDescription: "",
            ruleWeight: 100,
            ruleActive: true,
            ruleStore: true,
            profileModalVisible: false,
            selectedProfile: null,
        };
    }

    componentDidMount = async () => {
        this.setState({ loading: true });
        await this.loadTemplates();
        this.setState({ loading: false });
    }

    private loadTemplates = async () => {
        const response = await loadData<TemplatesResponse>("templates", new Map());
        if (response.status === 200 && response.data) {
            this.setState({
                wafTemplates: response.data.waf,
                jailTemplates: response.data.jail,
                profiles: response.data.profiles,
            });
        }
    }

    // --- Rule Template apply modal ---

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

        if (template.requires_fqdn && !this.state.fqdn.trim()) {
            message.error(`This template requires an FQDN (e.g. ${template.name.toLowerCase().replace(/[^a-z0-9]/g, '-')}.example.com)`);
            return;
        }

        this.setState({ applying: template.name, modalVisible: false });
        try {
            const body: Record<string, any> = {
                name: this.state.ruleName,
                description: this.state.ruleDescription,
                mode: template.pipeline === "jail" ? "log_only" : "enforce",
                weight: this.state.ruleWeight,
                allow: template.allow,
                store: this.state.ruleStore,
                pipeline: template.pipeline,
                path: template.path,
                query: template.query,
                country_code: template.country_code,
                fqdn: this.state.fqdn || null,
                ip_address: this.state.ipAddress || null,
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
                    ...(token ? { 'Authorization': `Bearer ${token}` } : {}),
                },
                body: JSON.stringify(body),
            });
            if (response.ok) {
                message.success(`Rule "${this.state.ruleName}" created successfully`);
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

    // --- Profile apply modal ---

    private openProfileApplyModal = (profile: RateLimitProfileTemplate) => {
        this.setState({
            profileModalVisible: true,
            selectedProfile: profile,
        });
    }

    private closeProfileModal = () => {
        this.setState({
            profileModalVisible: false,
            selectedProfile: null,
        });
    }

    private confirmApplyProfile = async () => {
        const profile = this.state.selectedProfile;
        if (!profile) return;

        this.setState({ applying: profile.name, profileModalVisible: false });
        try {
            const body: Record<string, any> = {
                name: profile.name,
                description: profile.description,
                max_retry: profile.max_retry,
                find_time_seconds: profile.find_time_seconds,
                ban_time_seconds: profile.ban_time_seconds,
                bantime_increment: profile.bantime_increment,
                bantime_multipliers: profile.bantime_multipliers,
                bantime_maxtime_seconds: profile.bantime_maxtime_seconds,
                ban_count_decay_days: profile.ban_count_decay_days,
                fail_codes: profile.fail_codes,
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
                message.success(`Rate limit profile "${profile.name}" created successfully`);
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

    private groupedByCategory = (templates: RuleTemplate[]): Map<string, RuleTemplate[]> => {
        const groups = new Map<string, RuleTemplate[]>();
        const searchLower = this.state.search.toLowerCase();
        const filtered = templates.filter(t =>
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

    private renderTemplateCard = (t: RuleTemplate) => {
        const isJail = t.pipeline === "jail";
        return (
            <Card
                key={t.name}
                size="small"
                variant="outlined"
                style={{ width: '48%', minWidth: 380, marginBottom: 8 }}
                actions={[
                    <Button
                        type="primary"
                        icon={<ThunderboltOutlined />}
                        loading={this.state.applying === t.name}
                        onClick={() => this.openApplyModal(t)}
                    >
                        Apply
                    </Button>,
                ]}
            >
                <Flex vertical gap="small">
                    <Flex justify="space-between" align="center">
                        <Text strong style={{ fontSize: 14 }}>{t.name}</Text>
                        <Tag color={SEVERITY_COLORS[t.severity] || "default"}>
                            {t.severity}
                        </Tag>
                    </Flex>
                    <Text type="secondary" style={{ fontSize: 12 }}>{t.description}</Text>
                    <Flex wrap gap={4} align="center">
                        {isJail ? (
                            <Tag icon={<LockOutlined />} color="orange">Jail</Tag>
                        ) : (
                            <Tag icon={t.allow ? <CheckCircleOutlined /> : <CloseCircleOutlined />}
                                color={t.allow ? "success" : "error"}>
                                {t.allow ? "Allow" : "Deny"}
                            </Tag>
                        )}
                        {t.path && <Tag color="blue">Path: {t.path}</Tag>}
                        {t.query && <Tag color="purple">Query: {t.query}</Tag>}
                        {t.country_code && <Tag color="cyan">Geo: {t.country_code}</Tag>}
                        {t.store && <Tag color="geekblue">Store</Tag>}
                    </Flex>
                    {isJail && t.rate_limit_profile_name && (
                        <Tag color="orange" style={{ fontSize: 11 }}>
                            Profile: {t.rate_limit_profile_name}
                        </Tag>
                    )}
                </Flex>
            </Card>
        );
    }

    private renderSection = (templates: RuleTemplate[], pipeline: string) => {
        const groups = this.groupedByCategory(templates);
        const isJail = pipeline === "jail";

        if (groups.size === 0) {
            return (
                <Empty
                    description={`No ${isJail ? "Jail" : "WAF"} templates found`}
                    style={{ padding: 48 }}
                />
            );
        }

        const collapseItems = Array.from(groups.entries())
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([category, templates]) => ({
                key: category,
                label: (
                    <Flex justify="space-between" style={{ width: '100%', paddingRight: 16 }}>
                        <Flex gap="small" align="center">
                            {CATEGORY_ICONS[category] || <AppstoreOutlined />}
                            <Text strong style={{ fontSize: 15 }}>
                                {CATEGORY_LABELS[category] || category.charAt(0).toUpperCase() + category.slice(1)}
                            </Text>
                        </Flex>
                        <Tag>{templates.length} template{templates.length !== 1 ? 's' : ''}</Tag>
                    </Flex>
                ),
                children: (
                    <Flex wrap gap="small" style={{ width: '100%' }}>
                        {templates.map(t => this.renderTemplateCard(t))}
                    </Flex>
                ),
            }));

        return (
            <Collapse
                items={collapseItems}
                defaultActiveKey={collapseItems.slice(0, 2).map(c => c.key)}
                size="small"
                style={{ background: 'transparent' }}
            />
        );
    }

    private renderProfilesSection = () => {
        const searchLower = this.state.search.toLowerCase();
        const filtered = this.state.profiles.filter(p =>
            p.name.toLowerCase().includes(searchLower) ||
            p.description.toLowerCase().includes(searchLower)
        );

        if (filtered.length === 0) {
            return (
                <Empty
                    description="No rate limit profiles found"
                    style={{ padding: 48 }}
                />
            );
        }

        return (
            <Flex wrap gap="small" style={{ width: '100%' }}>
                {filtered.map(p => (
                    <Card
                        key={p.id}
                        size="small"
                        variant="outlined"
                        style={{ width: '48%', minWidth: 380, marginBottom: 8 }}
                        actions={[
                            <Button
                                type="primary"
                                icon={<ThunderboltOutlined />}
                                loading={this.state.applying === p.name}
                                onClick={() => this.openProfileApplyModal(p)}
                            >
                                Apply
                            </Button>,
                        ]}
                    >
                        <Flex vertical gap="small">
                            <Text strong style={{ fontSize: 14 }}>{p.name}</Text>
                            <Text type="secondary" style={{ fontSize: 12 }}>{p.description}</Text>
                            <Flex wrap gap={4} align="center">
                                <Tag>Max retry: {p.max_retry}</Tag>
                                <Tag>Find time: {p.find_time_seconds}s</Tag>
                                <Tag>Ban time: {p.ban_time_seconds}s</Tag>
                                {p.bantime_increment && (
                                    <Tag color="orange">Escalation: {p.bantime_multipliers.join("×, ")}×</Tag>
                                )}
                                <Tag>Max ban: {p.bantime_maxtime_seconds >= 86400
                                    ? `${Math.floor(p.bantime_maxtime_seconds / 86400)}d`
                                    : `${p.bantime_maxtime_seconds}s`}</Tag>
                                <Tag>Decay: {p.ban_count_decay_days}d</Tag>
                                <Tag color="red">Fail: {p.fail_codes.join(", ")}</Tag>
                            </Flex>
                        </Flex>
                    </Card>
                ))}
            </Flex>
        );
    }

    render = () => {
        const selected = this.state.selectedTemplate;
        const selectedProfile = this.state.selectedProfile;

        const tabItems: TabsProps['items'] = [
            {
                key: 'waf',
                label: (
                    <Flex gap={4} align="center">
                        <CheckCircleOutlined style={{ color: '#52c41a' }} />
                        <span>WAF Templates</span>
                        <Tag style={{ marginLeft: 4 }}>{this.state.wafTemplates.length}</Tag>
                    </Flex>
                ),
                children: this.renderSection(this.state.wafTemplates, "waf"),
            },
            {
                key: 'jail',
                label: (
                    <Flex gap={4} align="center">
                        <LockOutlined style={{ color: '#fa8c16' }} />
                        <span>Jail Templates</span>
                        <Tag style={{ marginLeft: 4 }}>{this.state.jailTemplates.length}</Tag>
                    </Flex>
                ),
                children: this.renderSection(this.state.jailTemplates, "jail"),
            },
            {
                key: 'profiles',
                label: (
                    <Flex gap={4} align="center">
                        <WarningOutlined style={{ color: '#722ed1' }} />
                        <span>Rate Limit Profiles</span>
                        <Tag style={{ marginLeft: 4 }}>{this.state.profiles.length}</Tag>
                    </Flex>
                ),
                children: this.renderProfilesSection(),
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
                <Flex justify="space-between" align="center">
                    <Title level={3} style={{ margin: 0 }}>Templates</Title>
                    <Input
                        prefix={<SearchOutlined />}
                        placeholder="Search templates..."
                        style={{ width: 320 }}
                        value={this.state.search}
                        onChange={e => this.setState({ search: e.target.value })}
                        allowClear
                    />
                </Flex>
                <Text type="secondary">
                    Browse templates from your existing rules. Select a template to apply it as a new rule.
                    Templates are grouped by category and pipeline (WAF for allow/deny, Jail for rate limiting).
                </Text>
                <Tabs
                    defaultActiveKey="waf"
                    items={tabItems}
                    size="large"
                    onChange={key => this.setState({ activeTab: key })}
                />

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
                        <div style={{
                            background: "var(--color-bg-layout)",
                            borderRadius: 8,
                            border: "1px solid var(--color-border)",
                            padding: 16,
                        }}>
                            <Text strong style={{ fontSize: 15, marginBottom: 12, display: 'block' }}>
                                ⚡ Rule to be created
                            </Text>

                            {/* Pipeline + Action row */}
                            <Flex wrap gap="small" style={{ marginBottom: 12 }}>
                                {selected?.pipeline === "jail" ? (
                                    <Tag icon={<LockOutlined />} color="orange" style={{ fontSize: 13, padding: '2px 8px' }}>
                                        Pipeline: Jail
                                    </Tag>
                                ) : (
                                    <Tag color="blue" style={{ fontSize: 13, padding: '2px 8px' }}>
                                        Pipeline: WAF
                                    </Tag>
                                )}
                                {selected?.pipeline !== "jail" && (
                                    <Tag icon={selected?.allow ? <CheckCircleOutlined /> : <CloseCircleOutlined />}
                                        color={selected?.allow ? "success" : "error"}
                                        style={{ fontSize: 13, padding: '2px 8px' }}>
                                        Action: {selected?.allow ? "Allow" : "Deny"}
                                    </Tag>
                                )}
                                <Tag color="geekblue" style={{ fontSize: 13, padding: '2px 8px' }}>
                                    Store: {this.state.ruleStore ? "Yes" : "No"}
                                </Tag>
                                <Tag style={{ fontSize: 13, padding: '2px 8px' }}>
                                    Weight: {this.state.ruleWeight}
                                </Tag>
                                <Tag color={this.state.ruleActive ? "green" : "default"} style={{ fontSize: 13, padding: '2px 8px' }}>
                                    Active: {this.state.ruleActive ? "Yes" : "No"}
                                </Tag>
                                {selected?.pipeline !== "jail" && (
                                    <Tag color="purple" style={{ fontSize: 13, padding: '2px 8px' }}>
                                        Mode: Enforce
                                    </Tag>
                                )}
                                {selected?.pipeline === "jail" && (
                                    <Tag color="purple" style={{ fontSize: 13, padding: '2px 8px' }}>
                                        Mode: Log Only
                                    </Tag>
                                )}
                            </Flex>

                            {/* Filters section */}
                            <Text strong style={{ fontSize: 13, marginBottom: 8, display: 'block' }}>
                                🔍 Filters
                            </Text>
                            <Flex wrap gap="small" style={{ marginBottom: 12 }}>
                                {selected?.path ? (
                                    <Tag color="blue" style={{ fontSize: 12 }}>Path: {selected.path}</Tag>
                                ) : (
                                    <Tag style={{ fontSize: 12 }}>Path: —</Tag>
                                )}
                                {selected?.query ? (
                                    <Tag color="purple" style={{ fontSize: 12 }}>Query: {selected.query}</Tag>
                                ) : (
                                    <Tag style={{ fontSize: 12 }}>Query: —</Tag>
                                )}
                                {selected?.country_code ? (
                                    <Tag color="cyan" style={{ fontSize: 12 }}>Country: {selected.country_code}</Tag>
                                ) : (
                                    <Tag style={{ fontSize: 12 }}>Country: —</Tag>
                                )}
                            </Flex>

                            {/* Scope section */}
                            <Text strong style={{ fontSize: 13, marginBottom: 8, display: 'block' }}>
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
                            {selected?.pipeline === "jail" && selected?.rate_limit_profile_name && (
                                <>
                                    <div style={{ borderTop: "1px solid var(--color-border)", margin: '12px 0' }} />
                                    <Text strong style={{ fontSize: 13, marginBottom: 8, display: 'block' }}>
                                        ⏱ Rate Limit Profile
                                    </Text>
                                    <Tag icon={<LockOutlined />} color="orange" style={{ fontSize: 12 }}>
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
                                onChange={e => this.setState({ ruleName: e.target.value })}
                                addonBefore="Name"
                                allowClear
                            />
                            <Input
                                placeholder="Rule description"
                                value={this.state.ruleDescription}
                                onChange={e => this.setState({ ruleDescription: e.target.value })}
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
                                    onChange={value => this.setState({ ruleWeight: value ?? 100 })}
                                />
                            </Flex>
                            <Flex align="center" gap="small">
                                <Text style={{ width: 100, flexShrink: 0 }}>Active</Text>
                                <Switch
                                    checked={this.state.ruleActive}
                                    onChange={checked => this.setState({ ruleActive: checked })}
                                />
                                <Text style={{ width: 60, flexShrink: 0, marginLeft: 16 }}>Store</Text>
                                <Switch
                                    checked={this.state.ruleStore}
                                    onChange={checked => this.setState({ ruleStore: checked })}
                                />
                            </Flex>
                        </Flex>

                        {/* ── Scope inputs ── */}
                        <Flex vertical gap="small">
                            <Text strong>Configure scope</Text>
                            {selected?.requires_fqdn ? (
                                <>
                                    <Text type="warning" style={{ color: '#fa8c16', fontSize: 12 }}>
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
                                    <Text type="secondary" style={{ fontSize: 12 }}>
                                        This is a general template. Leave empty to apply to all traffic,
                                        or specify a specific FQDN to scope it.
                                    </Text>
                                    <Input
                                        placeholder="FQDN (optional)"
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
                    </Flex>
                </Modal>

                {/* Profile Template Apply modal */}
                <Modal
                    title={`Apply Profile: ${selectedProfile?.name || ""}`}
                    open={this.state.profileModalVisible}
                    onOk={this.confirmApplyProfile}
                    onCancel={this.closeProfileModal}
                    okText="Create Profile"
                    cancelText="Cancel"
                    width={560}
                >
                    <Flex vertical gap="middle">
                        <Text type="secondary">{selectedProfile?.description}</Text>

                        {/* ── Preview: what will be created ── */}
                        <div style={{
                            background: "var(--color-bg-layout)",
                            borderRadius: 8,
                            border: "1px solid var(--color-border)",
                            padding: 16,
                        }}>
                            <Text strong style={{ fontSize: 15, marginBottom: 12, display: 'block' }}>
                                ⚡ Rate limit profile to be created
                            </Text>

                            <Flex vertical gap="small">
                                <Flex align="center" gap="small">
                                    <Text strong style={{ width: 100 }}>Max retry:</Text>
                                    <Tag>{selectedProfile?.max_retry} requests</Tag>
                                </Flex>
                                <Flex align="center" gap="small">
                                    <Text strong style={{ width: 100 }}>Find time:</Text>
                                    <Tag>{selectedProfile?.find_time_seconds}s</Tag>
                                </Flex>
                                <Flex align="center" gap="small">
                                    <Text strong style={{ width: 100 }}>Ban time:</Text>
                                    <Tag>{selectedProfile?.ban_time_seconds}s</Tag>
                                </Flex>
                                <Flex align="center" gap="small">
                                    <Text strong style={{ width: 100 }}>Escalation:</Text>
                                    {selectedProfile?.bantime_increment ? (
                                        <Tag color="orange">Enabled ({selectedProfile?.bantime_multipliers.join("×, ")}×)</Tag>
                                    ) : (
                                        <Tag>Disabled</Tag>
                                    )}
                                </Flex>
                                <Flex align="center" gap="small">
                                    <Text strong style={{ width: 100 }}>Max ban:</Text>
                                    <Tag>{(selectedProfile?.bantime_maxtime_seconds ?? 0) >= 86400
                                        ? `${Math.floor((selectedProfile?.bantime_maxtime_seconds ?? 0) / 86400)}d`
                                        : `${selectedProfile?.bantime_maxtime_seconds ?? 0}s`}</Tag>
                                </Flex>
                                <Flex align="center" gap="small">
                                    <Text strong style={{ width: 100 }}>Decay:</Text>
                                    <Tag>{selectedProfile?.ban_count_decay_days}d</Tag>
                                </Flex>
                                <Flex align="center" gap="small">
                                    <Text strong style={{ width: 100 }}>Fail codes:</Text>
                                    <Tag color="red">{selectedProfile?.fail_codes.join(", ")}</Tag>
                                </Flex>
                            </Flex>
                        </div>

                        <Text style={{ fontSize: 12 }}>
                            This will create a new rate limit profile with the settings above.
                            You can then assign it to rules from the Rules page.
                        </Text>
                    </Flex>
                </Modal>
            </Flex>
        );
    }
}