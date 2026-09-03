import react from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Flex, Typography, Avatar, Card, Button } from 'antd';
import { CheckCircleOutlined, CloseCircleOutlined, SafetyOutlined } from '@ant-design/icons';

import { loadData } from "@/common/utils";
import {VERSION} from "@/constants";
import Logo from "@/assets/logo.svg";
import type { RuleTemplate, TemplatesResponse } from "@/models/template";

const TITLE = `Shuul (${VERSION})`;

interface Props {
    navigate: any
    t: any
}

interface State {
    loading: boolean;
    total_rules: number,
    total_active_rules: number,
    total_requests: number,
    total_filtered_requests: number,
    total_active_bans: number,
    // Security Checklist
    mustHaveTemplates: RuleTemplate[];
    appliedMustHave: Set<string>;
    rules: any[];
}


export class InnerPage extends react.Component<Props, State> {

    constructor(props: Props) {
        super(props);
        this.state = {
            loading: true,
            total_rules: 0,
            total_active_rules: 0,
            total_requests: 0,
            total_filtered_requests: 0,
            total_active_bans: 0,
            mustHaveTemplates: [],
            appliedMustHave: new Set(),
            rules: [],
        }
    }
    componentDidMount = async () => {

        const [rules_info_res, total_requests_res, total_filtered_requests_res, total_active_bans_res, templates_res, rules_res] = await Promise.all([
            loadData("rules/info/all"),
            loadData("stats/info", new Map([["option", "total"]])),
            loadData("stats/info", new Map([["option", "filtered"]])),
            loadData("bans/info", new Map()),
            loadData("templates"),
            loadData("rules"),
        ]);
        console.log("Totals loaded:", rules_info_res, total_requests_res, total_filtered_requests_res, total_active_bans_res);

        // Process security checklist data
        let mustHaveTemplates: RuleTemplate[] = [];
        let appliedMustHave = new Set<string>();
        let rules: any[] = [];

        if (templates_res.status === 200 && templates_res.data) {
            const data = templates_res.data as TemplatesResponse;
            mustHaveTemplates = [...data.waf, ...data.jail].filter(t => t.must_have);
        }

        if (rules_res.status === 200 && rules_res.data) {
            rules = rules_res.data as any[];
            // Check which must-have templates are already applied
            for (const t of mustHaveTemplates) {
                const isApplied = rules.some((r: any) =>
                    r.name && (r.name.includes(t.name) || t.name.includes(r.name))
                );
                if (isApplied) {
                    appliedMustHave.add(t.name);
                }
            }
        }

        this.setState({
            loading: false,
            total_rules: rules_info_res.status === 200 ? (rules_info_res.data as any).total : 0,
            total_active_rules: rules_info_res.status === 200 ? (rules_info_res.data as any).active : 0,
            total_requests: total_requests_res.status === 200 ? total_requests_res.data as number : 0,
            total_filtered_requests: total_filtered_requests_res.status === 200 ? total_filtered_requests_res.data as number : 0,
            total_active_bans: total_active_bans_res.status === 200 ? total_active_bans_res.data as number : 0,
            mustHaveTemplates,
            appliedMustHave,
            rules,
        });

    }

    render = () => {
        const { mustHaveTemplates, appliedMustHave } = this.state;
        const totalCount = mustHaveTemplates.length;
        const appliedCount = mustHaveTemplates.filter(t => appliedMustHave.has(t.name)).length;

        return (
            <Flex vertical justify="center" align="center" gap="middle" style={{ height: '100vh' }}>
                <Card loading={this.state.loading} style={{ minWidth: 600 }}>
                    <Card.Meta
                        avatar={<Avatar src={Logo} />}
                        title={<Typography.Title level={3} style={{ margin: 0 }}>{TITLE}</Typography.Title>}
                        description={
                            <>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5, cursor: "pointer" }}
                                    onClick={() => this.props.navigate("/admin/rules")}
                                >
                                    {`${this.props.t("Total of rules")}: ${this.state.total_rules}`}
                                </Typography.Title>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5, cursor: "pointer" }}
                                    onClick={() => this.props.navigate("/admin/rules")}
                                >
                                    {`${this.props.t("Total of active rules")}: ${this.state.total_active_rules}`}
                                </Typography.Title>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5 }}
                                >
                                    {`${this.props.t("Total of requests")}: ${this.state.total_requests}`}
                                </Typography.Title>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5 }}
                                >
                                    {`${this.props.t("Total of filtered requests")}: ${this.state.total_filtered_requests}`}
                                </Typography.Title>
                                <Typography.Title
                                    level={4}
                                    style={{ margin: 5, cursor: "pointer" }}
                                    onClick={() => this.props.navigate("/admin/bans")}
                                >
                                    {`${this.props.t("Active bans")}: ${this.state.total_active_bans}`}
                                </Typography.Title>
                            </>
                        }
                    />
                </Card>

                {/* Security Checklist */}
                <Card loading={this.state.loading} style={{ minWidth: 600 }}>
                    <Flex vertical gap="small">
                        <Flex align="center" gap="small">
                            <SafetyOutlined style={{ color: '#ff4d4f', fontSize: 20 }} />
                            <Typography.Title level={4} style={{ margin: 0 }}>🔴 Security Checklist</Typography.Title>
                        </Flex>
                        <Typography.Text type="secondary">
                            Must-have templates protect against critical threats. Apply them to secure your site.
                        </Typography.Text>
                        <div style={{ marginTop: 8 }}>
                            <Typography.Title level={3} style={{ textAlign: 'center', margin: '8px 0' }}>
                                {appliedCount}/{totalCount} must-have rules applied
                            </Typography.Title>
                            {/* Progress bar */}
                            <div style={{
                                height: 8,
                                background: '#f0f0f0',
                                borderRadius: 4,
                                overflow: 'hidden',
                                marginBottom: 16
                            }}>
                                <div style={{
                                    height: '100%',
                                    width: `${totalCount > 0 ? (appliedCount / totalCount) * 100 : 0}%`,
                                    background: appliedCount === totalCount ? '#52c41a' : '#fa8c16',
                                    borderRadius: 4,
                                    transition: 'width 0.3s'
                                }} />
                            </div>
                            {/* List of must-have templates */}
                            {mustHaveTemplates.map(t => (
                                <Flex key={t.name} justify="space-between" align="center" style={{ padding: '8px 0', borderBottom: '1px solid #f0f0f0' }}>
                                    <Flex gap="small" align="center">
                                        {appliedMustHave.has(t.name) ? (
                                            <CheckCircleOutlined style={{ color: '#52c41a' }} />
                                        ) : (
                                            <CloseCircleOutlined style={{ color: '#ff4d4f' }} />
                                        )}
                                        <Typography.Text>{t.name}</Typography.Text>
                                    </Flex>
                                    {!appliedMustHave.has(t.name) && (
                                        <Button size="small" type="primary" onClick={() => this.props.navigate("/admin/templates")}>
                                            Apply
                                        </Button>
                                    )}
                                </Flex>
                            ))}
                            {totalCount === 0 && (
                                <Typography.Text type="secondary" style={{ display: 'block', textAlign: 'center', padding: 16 }}>
                                    No must-have templates available.
                                </Typography.Text>
                            )}
                        </div>
                    </Flex>
                </Card>
            </Flex >

        );
    }
}

export default function DashboardPage() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}
