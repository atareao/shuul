import { useState } from "react";
import { Modal, Typography, Flex, Descriptions, Tabs, Button } from "antd";
import { PlusOutlined } from '@ant-design/icons';
import type Item from "@/models/record";
import CreateRuleFromRequestDialog from './create_rule_from_request_dialog';

const { Text } = Typography;

interface Props {
    record: Item | null;
    onClose: () => void;
    t: (key: string) => string;
}

export default function RequestDetailDialog({ record, onClose, t }: Props) {
    const [showCreateRule, setShowCreateRule] = useState(false);

    if (!record) return null;

    const formatValue = (val: any): string => {
        if (val === null || val === undefined) return "-";
        if (val instanceof Date) return val.toLocaleString();
        if (typeof val === 'string' && val.length === 0) return "-";
        return String(val);
    };

    const generalItems = [
        { label: t("Created At"), value: record.created_at },
        { label: t("IP Address"), value: record.ip_address },
        { label: t("FQDN"), value: record.fqdn },
        { label: t("Path"), value: record.path },
        { label: t("User Agent"), value: record.user_agent },
        { label: t("Country"), value: record.country_name },
        { label: t("Rule"), value: record.rule_name || (record.rule_id ? `#${record.rule_id}` : "-") },
    ];

    const detailItems = [
        { label: t("Protocol"), value: record.protocol },
        { label: t("Query"), value: record.query },
        { label: t("Method"), value: record.method },
        { label: t("Referer"), value: record.referer },
        { label: t("Content Type"), value: record.content_type },
        { label: t("Accept Language"), value: record.accept_language },
        { label: t("X-Request-ID"), value: record.x_request_id },
        { label: t("City Name"), value: record.city_name },
        { label: t("Country Code"), value: record.country_code },
    ];

    return (
        <>
            <CreateRuleFromRequestDialog
                record={showCreateRule ? record : null}
                onClose={() => setShowCreateRule(false)}
                t={t}
            />
            <Modal
                title={t("Request Details")}
                open={record !== null}
                onCancel={onClose}
                footer={
                    <Flex justify="space-between">
                        <Button
                            type="primary"
                            icon={<PlusOutlined />}
                            onClick={() => setShowCreateRule(true)}
                        >
                            {t("Create Rule from Request")}
                        </Button>
                        <Button onClick={onClose}>{t("Close")}</Button>
                    </Flex>
                }
                width={700}
                styles={{ body: { maxHeight: "60vh", overflowY: "auto" } }}
            >
                <Tabs
                    items={[
                        {
                            key: "general",
                            label: t("General"),
                            children: (
                                <Descriptions column={1} bordered size="small" style={{ marginTop: 16 }}>
                                    {generalItems.map(item => (
                                        <Descriptions.Item key={item.label} label={item.label} span={3}>
                                            <Text style={{ wordBreak: 'break-all' }}>{formatValue(item.value)}</Text>
                                        </Descriptions.Item>
                                    ))}
                                </Descriptions>
                            ),
                        },
                        {
                            key: "details",
                            label: t("Details"),
                            children: (
                                <Descriptions column={1} bordered size="small" style={{ marginTop: 16 }}>
                                    {detailItems.map(item => (
                                        <Descriptions.Item key={item.label} label={item.label} span={3}>
                                            <Text style={{ wordBreak: 'break-all' }}>{formatValue(item.value)}</Text>
                                        </Descriptions.Item>
                                    ))}
                                </Descriptions>
                            ),
                        },
                    ]}
                />
            </Modal>
        </>
    );
}