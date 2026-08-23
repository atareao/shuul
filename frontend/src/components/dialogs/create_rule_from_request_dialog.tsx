import { useState } from "react";
import { Modal, Typography, Flex, InputNumber, Radio, Tag, Alert, Space } from "antd";
import { BASE_URL } from '@/constants';
import type Item from "@/models/record";

const { Text } = Typography;

type PresetOption = "fqdn" | "path" | "query" | "fqdn_path" | "custom";

interface Props {
    record: Item | null;
    onClose: (created?: boolean) => void;
    t: (key: string) => string;
}

interface CheckedFields {
    ip_address: boolean;
    protocol: boolean;
    fqdn: boolean;
    path: boolean;
    query: boolean;
    city_name: boolean;
    country_name: boolean;
    country_code: boolean;
}

const defaultChecked: CheckedFields = {
    ip_address: false,
    protocol: false,
    fqdn: false,
    path: false,
    query: false,
    city_name: false,
    country_name: false,
    country_code: false,
};

const presetFields: Record<PresetOption, CheckedFields> = {
    fqdn: { ...defaultChecked, fqdn: true },
    path: { ...defaultChecked, path: true },
    query: { ...defaultChecked, query: true },
    fqdn_path: { ...defaultChecked, fqdn: true, path: true },
    custom: { ...defaultChecked },
};

export default function CreateRuleFromRequestDialog({ record, onClose, t }: Props) {
    const [preset, setPreset] = useState<PresetOption>("fqdn");
    const [custom, setCustom] = useState<CheckedFields>({ ...defaultChecked });
    const [weight, setWeight] = useState<number>(100);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const selectedFields = preset === "custom" ? custom : presetFields[preset];

    const toggleCustom = (field: keyof CheckedFields) => {
        setCustom(prev => ({ ...prev, [field]: !prev[field] }));
    };

    const handleCreate = async () => {
        if (!record) return;
        setLoading(true);
        setError(null);

        const body: Record<string, any> = {
            weight,
            allow: true,
            store: true,
            active: true,
            rate_limit_enabled: false,
            max_retry: 5,
            find_time_seconds: 600,
            ban_time_seconds: 3600,
        };

        const fieldMap: [keyof CheckedFields, string][] = [
            ["ip_address", "ip_address"],
            ["protocol", "protocol"],
            ["fqdn", "fqdn"],
            ["path", "path"],
            ["query", "query"],
            ["city_name", "city_name"],
            ["country_name", "country_name"],
            ["country_code", "country_code"],
        ];

        for (const [key, apiKey] of fieldMap) {
            if (selectedFields[key] && record[key] !== undefined && record[key] !== null) {
                body[apiKey] = record[key];
            }
        }

        const token = localStorage.getItem("token");

        try {
            const response = await fetch(`${BASE_URL}/api/v1/rules`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    Accept: "application/json",
                    ...(token ? { Authorization: `Bearer ${token}` } : {}),
                },
                body: JSON.stringify(body),
            });

            if (!response.ok) {
                const errBody = await response.json().catch(() => ({}));
                throw new Error(errBody.message || `Error HTTP: ${response.status}`);
            }

            onClose(true);
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        } finally {
            setLoading(false);
        }
    };

    const handleCancel = () => {
        onClose(false);
    };

    const presetOptions: { value: PresetOption; label: string }[] = [
        { value: "fqdn", label: "FQDN" },
        { value: "path", label: "Path" },
        { value: "query", label: "Query" },
        { value: "fqdn_path", label: "FQDN + Path" },
        { value: "custom", label: t("Custom") },
    ];

    const fieldLabels: { key: keyof CheckedFields; label: string }[] = [
        { key: "ip_address", label: "IP Address" },
        { key: "protocol", label: "Protocol" },
        { key: "fqdn", label: "FQDN" },
        { key: "path", label: "Path" },
        { key: "query", label: "Query" },
        { key: "city_name", label: "City Name" },
        { key: "country_name", label: "Country Name" },
        { key: "country_code", label: "Country Code" },
    ];

    if (!record) return null;

    return (
        <Modal
            title={t("Create Rule from Request")}
            open={record !== null}
            onOk={handleCreate}
            onCancel={handleCancel}
            okText={t("Create Rule")}
            cancelText={t("Cancel")}
            confirmLoading={loading}
            width={520}
        >
            <Flex vertical gap="middle">
                {error && (
                    <Alert message={error} type="error" showIcon closable onClose={() => setError(null)} />
                )}

                {/* Request data reference */}
                <Flex vertical gap={4}>
                    <Text strong>{t("Request Data")}</Text>
                    <Flex wrap gap={4}>
                        {fieldLabels.map(({ key, label }) =>
                            record[key] ? (
                                <Tag key={key} color="blue">
                                    {label}: {record[key]?.toString().substring(0, 40)}
                                </Tag>
                            ) : null
                        )}
                    </Flex>
                </Flex>

                {/* Preset selector */}
                <Flex vertical gap={4}>
                    <Text strong>{t("Rule Pattern")}</Text>
                    <Radio.Group
                        value={preset}
                        onChange={(e) => {
                            setPreset(e.target.value);
                            if (e.target.value === "custom") {
                                setCustom({ ...presetFields[preset] });
                            }
                        }}
                    >
                        <Space direction="vertical">
                            {presetOptions.map((opt) => (
                                <Radio key={opt.value} value={opt.value}>
                                    {opt.label}
                                </Radio>
                            ))}
                        </Space>
                    </Radio.Group>
                </Flex>

                {/* Custom field toggles */}
                {preset === "custom" && (
                    <Flex vertical gap={4}>
                        <Text strong>{t("Select Fields")}</Text>
                        <Space wrap>
                            {fieldLabels.map(({ key, label }) => (
                                <Tag
                                    key={key}
                                    color={custom[key] ? "green" : "default"}
                                    style={{ cursor: "pointer" }}
                                    onClick={() => toggleCustom(key)}
                                >
                                    {label}
                                    {record[key] !== undefined && record[key] !== null
                                        ? `: ${record[key]?.toString().substring(0, 30)}`
                                        : ""}
                                </Tag>
                            ))}
                        </Space>
                    </Flex>
                )}

                {/* Preview */}
                <Flex vertical gap={4}>
                    <Text strong>{t("Rule will match on")}</Text>
                    <Space wrap>
                        {fieldLabels
                            .filter(({ key }) => selectedFields[key])
                            .map(({ key, label }) => (
                                <Tag key={key} color="green">
                                    {label}: {record[key]?.toString().substring(0, 40)}
                                </Tag>
                            ))}
                        {!fieldLabels.some(({ key }) => selectedFields[key]) && (
                            <Text type="secondary">{t("No fields selected — rule will match all requests")}</Text>
                        )}
                    </Space>
                </Flex>

                {/* Weight */}
                <Flex align="center" gap="small">
                    <Text style={{ width: 80 }}>{t("Weight")}:</Text>
                    <InputNumber
                        min={1}
                        max={99999}
                        value={weight}
                        onChange={(v) => setWeight(v ?? 100)}
                        style={{ width: 120 }}
                    />
                </Flex>
            </Flex>
        </Modal>
    );
}