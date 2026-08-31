import { useState } from "react";
import { Modal, Typography, Flex, InputNumber, Tag, Alert, Space, Tabs, Input, Switch } from "antd";
import { BASE_URL } from '@/constants';
import type Item from "@/models/record";

const { Text } = Typography;

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

export default function CreateRuleFromRequestDialog({ record, onClose, t }: Props) {
    const [checkedFields, setCheckedFields] = useState<CheckedFields>({ ...defaultChecked });
    const [weight, setWeight] = useState<number>(100);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const [active, setActive] = useState<boolean>(true);
    const [allow, setAllow] = useState<boolean>(true);
    const [store, setStore] = useState<boolean>(true);

    const [rateLimitEnabled, setRateLimitEnabled] = useState<boolean>(false);
    const [maxRetry, setMaxRetry] = useState<number>(5);
    const [findTimeSeconds, setFindTimeSeconds] = useState<number>(600);
    const [banTimeSeconds, setBanTimeSeconds] = useState<number>(3600);
    const [bantimeIncrement, setBantimeIncrement] = useState<boolean>(false);
    const [bantimeMultipliers, setBantimeMultipliers] = useState<string>("");
    const [bantimeMaxtimeSeconds, setBantimeMaxtimeSeconds] = useState<number>(604800);
    const [banCountDecayDays, setBanCountDecayDays] = useState<number>(30);
    const [ignoreip, setIgnoreip] = useState<string>("");
    const [webhook, setWebhook] = useState<string>("");

    const selectedFields = checkedFields;

    const toggleField = (field: keyof CheckedFields) => {
        setCheckedFields(prev => ({ ...prev, [field]: !prev[field] }));
    };

    const handleCreate = async () => {
        if (!record) return;
        setLoading(true);
        setError(null);

        const body: Record<string, any> = {
            weight,
            allow,
            store,
            active,
            rate_limit_enabled: rateLimitEnabled,
            max_retry: maxRetry,
            find_time_seconds: findTimeSeconds,
            ban_time_seconds: banTimeSeconds,
            bantime_increment: bantimeIncrement,
            bantime_multipliers: bantimeMultipliers
                ? bantimeMultipliers.split(",").map(s => Number(s.trim())).filter(n => !isNaN(n))
                : [],
            bantime_maxtime_seconds: bantimeMaxtimeSeconds,
            ban_count_decay_days: banCountDecayDays,
            ignoreip: ignoreip
                ? ignoreip.split(",").map(s => s.trim()).filter(s => s.length > 0)
                : [],
            webhook,
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
            width={600}
        >
            <Flex vertical gap="middle">
                {error && (
                    <Alert message={error} type="error" showIcon closable onClose={() => setError(null)} />
                )}

                <Tabs
                    items={[
                        {
                            key: "general",
                            label: t("General"),
                            children: (
                                <Flex vertical gap="middle">
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

                                    {/* Field toggles — always visible */}
                                    <Flex vertical gap={4}>
                                        <Text strong>{t("Select Fields")}</Text>
                                        <Space wrap>
                                            {fieldLabels.map(({ key, label }) => (
                                                <Tag
                                                    key={key}
                                                    color={checkedFields[key] ? "green" : "default"}
                                                    style={{ cursor: "pointer" }}
                                                    onClick={() => toggleField(key)}
                                                >
                                                    {label}
                                                    {record[key] !== undefined && record[key] !== null
                                                        ? `: ${record[key]?.toString().substring(0, 30)}`
                                                        : ""}
                                                </Tag>
                                            ))}
                                        </Space>
                                    </Flex>

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

                                    {/* Active */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Active")}:</Text>
                                        <Switch checked={active} onChange={setActive} />
                                    </Flex>

                                    {/* Allow */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Allow")}:</Text>
                                        <Switch checked={allow} onChange={setAllow} />
                                    </Flex>

                                    {/* Store */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Store")}:</Text>
                                        <Switch checked={store} onChange={setStore} />
                                    </Flex>

                                    {/* Weight */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Weight")}:</Text>
                                        <InputNumber
                                            min={1}
                                            max={99999}
                                            value={weight}
                                            onChange={(v) => setWeight(v ?? 100)}
                                            style={{ width: 120 }}
                                        />
                                    </Flex>
                                </Flex>
                            ),
                        },
                        {
                            key: "rate_limit",
                            label: t("Rate Limit"),
                            children: (
                                <Flex vertical gap="middle">
                                    {/* rate_limit_enabled */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Rate Limit Enabled")}:</Text>
                                        <Switch checked={rateLimitEnabled} onChange={setRateLimitEnabled} />
                                    </Flex>

                                    {/* max_retry */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Max Retry")}:</Text>
                                        <InputNumber
                                            min={1}
                                            max={99999}
                                            value={maxRetry}
                                            onChange={(v) => setMaxRetry(v ?? 5)}
                                            style={{ width: 120 }}
                                        />
                                    </Flex>

                                    {/* find_time_seconds */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Find Time")}:</Text>
                                        <InputNumber
                                            min={1}
                                            max={999999}
                                            value={findTimeSeconds}
                                            onChange={(v) => setFindTimeSeconds(v ?? 600)}
                                            style={{ width: 120 }}
                                        />
                                    </Flex>

                                    {/* ban_time_seconds */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Ban Time")}:</Text>
                                        <InputNumber
                                            min={1}
                                            max={999999}
                                            value={banTimeSeconds}
                                            onChange={(v) => setBanTimeSeconds(v ?? 3600)}
                                            style={{ width: 120 }}
                                        />
                                    </Flex>

                                    {/* bantime_increment */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Bantime Increment")}:</Text>
                                        <Switch checked={bantimeIncrement} onChange={setBantimeIncrement} />
                                    </Flex>

                                    {/* bantime_multipliers */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Bantime Multipliers")}:</Text>
                                        <Input
                                            value={bantimeMultipliers}
                                            onChange={(e) => setBantimeMultipliers(e.target.value)}
                                            placeholder="e.g. 1,2,4"
                                            style={{ width: 200 }}
                                        />
                                    </Flex>

                                    {/* bantime_maxtime_seconds */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Bantime Max Time")}:</Text>
                                        <InputNumber
                                            min={1}
                                            max={9999999}
                                            value={bantimeMaxtimeSeconds}
                                            onChange={(v) => setBantimeMaxtimeSeconds(v ?? 604800)}
                                            style={{ width: 120 }}
                                        />
                                    </Flex>

                                    {/* ban_count_decay_days */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Ban Count Decay Days")}:</Text>
                                        <InputNumber
                                            min={1}
                                            max={9999}
                                            value={banCountDecayDays}
                                            onChange={(v) => setBanCountDecayDays(v ?? 30)}
                                            style={{ width: 120 }}
                                        />
                                    </Flex>

                                    {/* ignoreip */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Ignore IP")}:</Text>
                                        <Input
                                            value={ignoreip}
                                            onChange={(e) => setIgnoreip(e.target.value)}
                                            placeholder="e.g. 127.0.0.1,192.168.1.1"
                                            style={{ width: 250 }}
                                        />
                                    </Flex>

                                    {/* webhook */}
                                    <Flex align="center" gap="small">
                                        <Text style={{ width: 100 }}>{t("Webhook")}:</Text>
                                        <Input
                                            value={webhook}
                                            onChange={(e) => setWebhook(e.target.value)}
                                            placeholder="https://..."
                                            style={{ width: 250 }}
                                        />
                                    </Flex>
                                </Flex>
                            ),
                        },
                    ]}
                />
            </Flex>
        </Modal>
    );
}