import { useEffect, useMemo, useState } from "react";
import { Alert, Button, Card, Descriptions, Flex, Input, InputNumber, Modal, Space, Tabs, Tag, Typography } from "antd";
import { useTranslation } from "react-i18next";

import { BASE_URL, VERSION } from "@/constants";
import type { DialogMode } from "@/common/types";
import type RequestRecord from "@/models/record";

const { Text } = Typography;

type RulePayload = {
    active: boolean;
    allow: boolean;
    store: boolean;
    weight: number;
    fqdn?: string | null;
    path?: string | null;
    query?: string | null;
};

type BanPayload = {
    ip_address: string;
    reason?: string;
    ban_duration_seconds?: number;
};

type FilterPreset = {
    key: string;
    label: string;
    fields: Array<"fqdn" | "path" | "query">;
};

interface Props {
    request?: RequestRecord;
    dialogMode?: DialogMode;
    onClose: () => void;
}

const FILTER_PRESETS: FilterPreset[] = [
    { key: "fqdn", label: "Filter by FQDN", fields: ["fqdn"] },
    { key: "path", label: "Filter by Path", fields: ["path"] },
    { key: "query", label: "Filter by Query", fields: ["query"] },
    { key: "fqdn-path", label: "Filter by FQDN + Path", fields: ["fqdn", "path"] },
    { key: "fqdn-query", label: "Filter by FQDN + Query", fields: ["fqdn", "query"] },
    { key: "path-query", label: "Filter by Path + Query", fields: ["path", "query"] },
    { key: "fqdn-path-query", label: "Filter by FQDN + Path + Query", fields: ["fqdn", "path", "query"] },
];

const hasValue = (value?: string) => Boolean(value && value.trim() !== "");

const toNullable = (value?: string) => value?.trim() || null;

async function createResource<T>(endpoint: string, payload: T) {
    const response = await fetch(`${BASE_URL}/api/${VERSION}/${endpoint}`, {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "Accept": "application/json",
        },
        body: JSON.stringify(payload),
    });

    let content: { message?: string } = {};
    try {
        content = await response.json();
    } catch {
        content = {};
    }

    if (!response.ok) {
        throw new Error(content.message || `Error HTTP: ${response.status} - ${response.statusText}`);
    }
}

export default function RequestActionDialog({ request, dialogMode, onClose }: Props) {
    const { t } = useTranslation();
    const [activeTab, setActiveTab] = useState("filters");
    const [submittingKey, setSubmittingKey] = useState<string>();
    const [errorMessage, setErrorMessage] = useState<string>();
    const [banReason, setBanReason] = useState("");
    const [banDuration, setBanDuration] = useState<number>(3600);

    const availablePresets = useMemo(
        () =>
            FILTER_PRESETS.filter((preset) =>
                preset.fields.every((field) => hasValue(request?.[field])),
            ),
        [request],
    );

    useEffect(() => {
        setActiveTab(availablePresets.length > 0 ? "filters" : "ban");
        setSubmittingKey(undefined);
        setErrorMessage(undefined);
        setBanDuration(3600);
        setBanReason(
            request?.ip_address
                ? `Blocked from requests view (${request.ip_address})`
                : "Blocked from requests view",
        );
    }, [availablePresets.length, request?.id, request?.ip_address]);

    if (!request || !dialogMode) {
        return null;
    }

    const handleCreateRule = async (preset: FilterPreset) => {
        const payload: RulePayload = {
            active: true,
            allow: false,
            store: true,
            weight: 100,
        };

        for (const field of preset.fields) {
            payload[field] = toNullable(request[field]);
        }

        try {
            setSubmittingKey(preset.key);
            setErrorMessage(undefined);
            await createResource("rules", payload);
            onClose();
        } catch (error) {
            setErrorMessage(error instanceof Error ? error.message : String(error));
        } finally {
            setSubmittingKey(undefined);
        }
    };

    const handleBan = async () => {
        if (!hasValue(request.ip_address)) {
            setErrorMessage(t("The selected request does not contain a valid IP address"));
            return;
        }

        const payload: BanPayload = {
            ip_address: request.ip_address!.trim(),
            reason: banReason.trim() || undefined,
            ban_duration_seconds: banDuration,
        };

        try {
            setSubmittingKey("ban");
            setErrorMessage(undefined);
            await createResource("bans", payload);
            onClose();
        } catch (error) {
            setErrorMessage(error instanceof Error ? error.message : String(error));
        } finally {
            setSubmittingKey(undefined);
        }
    };

    const requestSummary = (
        <Descriptions size="small" column={1} bordered>
            {hasValue(request.ip_address) && (
                <Descriptions.Item label={t("IP Address")}>{request.ip_address}</Descriptions.Item>
            )}
            {hasValue(request.fqdn) && (
                <Descriptions.Item label={t("FQDN")}>{request.fqdn}</Descriptions.Item>
            )}
            {hasValue(request.path) && (
                <Descriptions.Item label={t("Path")}>{request.path}</Descriptions.Item>
            )}
            {hasValue(request.query) && (
                <Descriptions.Item label={t("Query")}>{request.query}</Descriptions.Item>
            )}
        </Descriptions>
    );

    return (
        <Modal
            title={t("Create action from request")}
            open={true}
            footer={null}
            onCancel={onClose}
            width={900}
            styles={{ body: { maxHeight: "70vh", overflowY: "auto" } }}
        >
            <Flex vertical gap="middle">
                {errorMessage && (
                    <Alert
                        message={errorMessage}
                        type="error"
                        showIcon
                        closable
                        onClose={() => setErrorMessage(undefined)}
                    />
                )}
                {requestSummary}
                <Tabs
                    activeKey={activeTab}
                    onChange={setActiveTab}
                    items={[
                        {
                            key: "filters",
                            label: t("Filter rules"),
                            children: (
                                <Flex vertical gap="middle">
                                    {availablePresets.length === 0 ? (
                                        <Alert
                                            type="info"
                                            showIcon
                                            message={t("There is not enough request data to build a filter rule")}
                                        />
                                    ) : (
                                        availablePresets.map((preset) => (
                                            <Card key={preset.key} size="small">
                                                <Flex justify="space-between" align="center" gap="middle" wrap>
                                                    <Flex vertical gap="small">
                                                        <Text strong>{t(preset.label)}</Text>
                                                        <Space wrap>
                                                            {preset.fields.map((field) => (
                                                                <Tag key={field}>
                                                                    {t(field.toUpperCase())}: {request[field]}
                                                                </Tag>
                                                            ))}
                                                        </Space>
                                                    </Flex>
                                                    <Button
                                                        type="primary"
                                                        onClick={() => void handleCreateRule(preset)}
                                                        loading={submittingKey === preset.key}
                                                    >
                                                        {t("Create rule")}
                                                    </Button>
                                                </Flex>
                                            </Card>
                                        ))
                                    )}
                                </Flex>
                            ),
                        },
                        {
                            key: "ban",
                            label: t("Ban"),
                            children: (
                                <Flex vertical gap="middle">
                                    {!hasValue(request.ip_address) ? (
                                        <Alert
                                            type="warning"
                                            showIcon
                                            message={t("This request does not include an IP address, so it cannot be banned")}
                                        />
                                    ) : (
                                        <>
                                            <Flex vertical gap="small">
                                                <Text strong>{t("Reason")}</Text>
                                                <Input
                                                    value={banReason}
                                                    onChange={(event) => setBanReason(event.target.value)}
                                                    placeholder={t("Manual ban")}
                                                />
                                            </Flex>
                                            <Flex vertical gap="small">
                                                <Text strong>{t("Duration (s)")}</Text>
                                                <InputNumber
                                                    min={1}
                                                    value={banDuration}
                                                    onChange={(value) => setBanDuration(value ?? 3600)}
                                                    style={{ width: "100%" }}
                                                />
                                            </Flex>
                                            <Button
                                                type="primary"
                                                danger
                                                onClick={() => void handleBan()}
                                                loading={submittingKey === "ban"}
                                            >
                                                {t("Ban IP")}
                                            </Button>
                                        </>
                                    )}
                                </Flex>
                            ),
                        },
                    ]}
                />
            </Flex>
        </Modal>
    );
}
