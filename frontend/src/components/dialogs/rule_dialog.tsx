import { useState, useEffect, useCallback } from "react";
import { Modal, Typography, Flex, Input, InputNumber, Switch, Alert, Tabs, Select } from "antd";
import type { TabsProps } from "antd";
import { GlobalOutlined, CloudServerOutlined, FileSearchOutlined } from '@ant-design/icons';
import { BASE_URL } from '@/constants';
import type { DialogMode } from '@/common/types';
import { DialogModes } from '@/common/types';
import type { DialogMessages } from '@/components/dialogs/custom_dialog';
import type Item from "@/models/rule";
import type RateLimitProfile from "@/models/rate_limit_profile";

const { Text } = Typography;

// Props del RuleDialog
interface RuleDialogProps {
    dialogMode: DialogMode;
    selectedItem: Item | undefined;
    handleCloseDialog: (item?: Item | undefined) => void;
    endpoint: string;
    dialogMessages?: DialogMessages;
    t: (key: string) => string;
}

// Valores por defecto para CREATE mode
const DEFAULT_VALUES: Record<string, any> = {
    active: true,
    allow: true,
    store: true,
    weight: 100,
    name: "",
    description: "",
    mode: "enforce",
    pipeline: "waf",
    ip_address: "",
    protocol: "",
    fqdn: "",
    path: "",
    query: "",
    city_name: "",
    country_name: "",
    country_code: "",
    user_agent: "",
    method: "",
    referer: "",
    content_type: "",
    accept_language: "",
    x_request_id: "",
    rate_limit_profile_id: undefined,
};

// Inicializa los valores del formulario desde un Item o desde valores por defecto
function initializeFromItem(item?: Item): Record<string, any> {
    if (!item) {
        return { ...DEFAULT_VALUES };
    }
    return {
        active: item.active !== undefined ? Boolean(item.active) : true,
        allow: item.allow !== undefined ? Boolean(item.allow) : true,
        store: item.store !== undefined ? Boolean(item.store) : true,
        weight: item.weight ?? 100,
        name: item.name ?? "",
        description: item.description ?? "",
        mode: item.mode ?? "enforce",
        pipeline: item.pipeline ?? "waf",
        ip_address: item.ip_address ?? "",
        protocol: item.protocol ?? "",
        fqdn: item.fqdn ?? "",
        path: item.path ?? "",
        query: item.query ?? "",
        city_name: item.city_name ?? "",
        country_name: item.country_name ?? "",
        country_code: item.country_code ?? "",
        user_agent: item.user_agent ?? "",
        method: item.method ?? "",
        referer: item.referer ?? "",
        content_type: item.content_type ?? "",
        accept_language: item.accept_language ?? "",
        x_request_id: item.x_request_id ?? "",
        rate_limit_profile_id: item.rate_limit_profile_id ?? undefined,
    };
}

// Convierte los valores del formulario al formato esperado por la API
function formatForApi(values: Record<string, any>): Record<string, any> {
    const body: Record<string, any> = {
        active: values.active,
        allow: values.allow,
        store: values.store,
        weight: values.weight,
        name: values.name || null,
        description: values.description || null,
        mode: values.mode || "enforce",
        pipeline: values.pipeline || "waf",
        ip_address: values.ip_address || null,
        protocol: values.protocol || null,
        fqdn: values.fqdn || null,
        path: values.path || null,
        query: values.query || null,
        city_name: values.city_name || null,
        country_name: values.country_name || null,
        country_code: values.country_code || null,
        user_agent: values.user_agent || null,
        method: values.method || null,
        referer: values.referer || null,
        content_type: values.content_type || null,
        accept_language: values.accept_language || null,
        x_request_id: values.x_request_id || null,
        rate_limit_profile_id: values.rate_limit_profile_id !== undefined && values.rate_limit_profile_id !== null
            ? Number(values.rate_limit_profile_id)
            : null,
    };
    return body;
}

export default function RuleDialog({
    dialogMode,
    selectedItem,
    handleCloseDialog,
    endpoint,
    dialogMessages,
    t,
}: RuleDialogProps) {
    const [formValues, setFormValues] = useState<Record<string, any>>(() =>
        initializeFromItem(selectedItem)
    );
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [profiles, setProfiles] = useState<RateLimitProfile[]>([]);
    const [profilesLoading, setProfilesLoading] = useState(false);

    // Load rate limit profiles for the selector
    useEffect(() => {
        const loadProfiles = async () => {
            setProfilesLoading(true);
            const token = localStorage.getItem("token");
            try {
                const response = await fetch(`${BASE_URL}/api/v1/rate-limit-profiles`, {
                    headers: {
                        "Content-Type": "application/json",
                        ...(token ? { Authorization: `Bearer ${token}` } : {}),
                    },
                });
                if (response.ok) {
                    const json = await response.json();
                    if (json.data) {
                        setProfiles(json.data);
                    }
                }
            } catch (_) {
                // Silently fail — profiles are optional
            } finally {
                setProfilesLoading(false);
            }
        };
        loadProfiles();
    }, []);

    // Actualizar formValues cuando cambia selectedItem o dialogMode
    useEffect(() => {
        if (dialogMode === DialogModes.CREATE) {
            setFormValues(initializeFromItem(undefined));
        } else if (selectedItem) {
            setFormValues(initializeFromItem(selectedItem));
        }
        setError(null);
    }, [selectedItem, dialogMode]);

    const updateField = useCallback((key: string, value: any) => {
        setFormValues(prev => ({ ...prev, [key]: value }));
    }, []);

    const handleApiCall = async () => {
        setLoading(true);
        setError(null);

        const token = localStorage.getItem("token");
        const basePath = `${BASE_URL}/api/v1/${endpoint}`;

        let method: string;
        let url: string;
        let body: string | null;

        if (dialogMode === DialogModes.DELETE) {
            method = "DELETE";
            url = `${basePath}?id=${selectedItem?.id}`;
            body = null;
        } else if (dialogMode === DialogModes.CREATE) {
            method = "POST";
            url = basePath;
            body = JSON.stringify(formatForApi(formValues));
        } else if (dialogMode === DialogModes.UPDATE) {
            method = "PATCH";
            url = basePath;
            const apiBody = formatForApi(formValues);
            apiBody.id = selectedItem?.id;
            body = JSON.stringify(apiBody);
        } else if (dialogMode === DialogModes.READ) {
            method = "GET";
            url = `${basePath}?id=${selectedItem?.id}`;
            body = null;
        } else {
            setLoading(false);
            return;
        }

        try {
            const response = await fetch(url, {
                method,
                headers: {
                    "Content-Type": "application/json",
                    Accept: "application/json",
                    ...(token ? { Authorization: `Bearer ${token}` } : {}),
                },
                body,
            });

            if (!response.ok) {
                let errorBody: { message?: string } = {};
                try {
                    errorBody = await response.json();
                } catch (_) { /* ignore */ }
                throw new Error(errorBody.message || `Error HTTP: ${response.status} - ${response.statusText}`);
            }

            const content = await response.json();
            handleCloseDialog(content.data as Item);
        } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            setError(msg);
        } finally {
            setLoading(false);
        }
    };

    const handleOk = async () => {
        if (dialogMode === DialogModes.DELETE) {
            await handleApiCall();
        } else {
            await handleApiCall();
        }
    };

    const handleCancel = () => {
        handleCloseDialog(undefined);
    };

    const disabled = dialogMode === DialogModes.READ;
    const isDelete = dialogMode === DialogModes.DELETE;
    const isCreate = dialogMode === DialogModes.CREATE;
    const isUpdate = dialogMode === DialogModes.UPDATE;
    const isFormMode = isCreate || isUpdate || dialogMode === DialogModes.READ;

    // Find selected profile for read-only info display
    const selectedProfile = profiles.find(
        p => p.id === formValues.rate_limit_profile_id
    ) ?? null;

    let title = "";
    let confirmMessage = "";

    if (isCreate) {
        title = t(dialogMessages?.createTitle || "Create Rule");
    } else if (isUpdate) {
        title = t(dialogMessages?.updateTitle || "Update Rule");
    } else if (dialogMode === DialogModes.READ) {
        title = t(dialogMessages?.readTitle || "View Rule");
    } else if (isDelete) {
        title = t(dialogMessages?.deleteTitle || "Delete Rule");
        confirmMessage = dialogMessages?.confirmDeleteMessage
            ? t(dialogMessages.confirmDeleteMessage(selectedItem?.id ?? ""))
            : t("Are you sure you want to delete this rule?");
    }

    // --- Render helpers ---

    const renderInputRow = (label: string, key: string, placeholder?: string) => (
        <Flex align="center" gap="small">
            <Text style={{ width: 120, flexShrink: 0 }}>{t(label)}</Text>
            <Input
                style={{ width: "100%" }}
                value={formValues[key] ?? ""}
                placeholder={placeholder || t(label)}
                onChange={(e) => updateField(key, e.target.value)}
                disabled={disabled}
            />
        </Flex>
    );

    const renderSelectRow = (label: string, key: string, options: { value: any; label: string }[], placeholder?: string) => (
        <Flex align="center" gap="small">
            <Text style={{ width: 120, flexShrink: 0 }}>{t(label)}</Text>
            <Select
                style={{ width: "100%" }}
                value={formValues[key] !== undefined && formValues[key] !== null ? formValues[key] : undefined}
                placeholder={placeholder || t(label)}
                onChange={(value) => updateField(key, value)}
                disabled={disabled}
                allowClear
                options={options}
                loading={profilesLoading}
            />
        </Flex>
    );

    // --- Tab contents ---

    const genTab = (
        <Flex vertical gap="middle" style={{ paddingTop: 16 }}>
            <Flex align="center" gap="small">
                <Text style={{ width: 120, flexShrink: 0 }}>{t("Active")}</Text>
                <Switch
                    checked={Boolean(formValues.active)}
                    onChange={(checked) => updateField("active", checked)}
                    disabled={disabled}
                />
                {formValues.pipeline !== "jail" && (
                    <>
                        <Text style={{ width: 60, flexShrink: 0, marginLeft: 16 }}>{t("Allow")}</Text>
                        <Switch
                            checked={Boolean(formValues.allow)}
                            onChange={(checked) => updateField("allow", checked)}
                            disabled={disabled}
                        />
                    </>
                )}
                <Text style={{ width: 60, flexShrink: 0, marginLeft: 16 }}>{t("Store")}</Text>
                <Switch
                    checked={Boolean(formValues.store)}
                    onChange={(checked) => updateField("store", checked)}
                    disabled={disabled}
                />
            </Flex>
            {renderSelectRow("Pipeline", "pipeline", [
                { value: "waf", label: "WAF" },
                { value: "jail", label: "Jail" },
            ])}
            <Flex align="center" gap="small">
                <Text style={{ width: 120, flexShrink: 0 }}>{t("Weight")}</Text>
                <InputNumber
                    style={{ width: "100%" }}
                    value={formValues.weight as number}
                    min={1}
                    max={99999}
                    onChange={(value) => updateField("weight", value ?? 100)}
                    disabled={disabled}
                />
            </Flex>
            {renderInputRow("Name", "name")}
            {renderInputRow("Description", "description")}
            {formValues.pipeline !== "jail" && renderSelectRow("Mode", "mode", [
                { value: "enforce", label: "Enforce" },
                { value: "log_only", label: "Log Only" },
                { value: "off", label: "Off" },
            ])}
            {formValues.pipeline !== "waf" && (
                <>
                    {renderSelectRow("Rate Limit Profile", "rate_limit_profile_id", [
                        { value: undefined, label: "None" },
                        ...profiles.map(p => ({ value: p.id, label: p.name })),
                    ])}
                    {selectedProfile && (
                        <Flex vertical gap={4} style={{
                            background: "var(--color-bg-layout)",
                            borderRadius: 6,
                            padding: "8px 12px",
                            fontSize: 12,
                            border: "1px solid var(--color-border)",
                        }}>
                            <Text strong style={{ fontSize: 13, marginBottom: 4 }}>
                                {selectedProfile.name}
                            </Text>
                            <Text type="secondary" style={{ fontSize: 11 }}>
                                {selectedProfile.description}
                            </Text>
                            <Flex wrap gap="small" style={{ marginTop: 4 }}>
                                <Text><Text strong>Max Retry:</Text> {selectedProfile.max_retry}</Text>
                                <Text><Text strong>Find Time:</Text> {selectedProfile.find_time_seconds}s</Text>
                                <Text><Text strong>Ban Time:</Text> {selectedProfile.ban_time_seconds}s</Text>
                                <Text><Text strong>Escalate:</Text> {selectedProfile.bantime_increment ? "Yes" : "No"}</Text>
                                <Text><Text strong>Max Ban:</Text> {selectedProfile.bantime_maxtime_seconds}s</Text>
                                <Text><Text strong>Decay:</Text> {selectedProfile.ban_count_decay_days}d</Text>
                                <Text><Text strong>Fail Codes:</Text> {selectedProfile.fail_codes?.join(", ")}</Text>
                            </Flex>
                        </Flex>
                    )}
                </>
            )}
        </Flex>
    );

    const networkTab = (
        <Flex vertical gap="middle" style={{ paddingTop: 16 }}>
            {renderInputRow("IP Address", "ip_address")}
            {renderInputRow("Protocol", "protocol")}
            {renderInputRow("FQDN", "fqdn")}
            {renderInputRow("Referer", "referer")}
        </Flex>
    );

    const locationTab = (
        <Flex vertical gap="middle" style={{ paddingTop: 16 }}>
            {renderInputRow("City Name", "city_name")}
            {renderInputRow("Country Name", "country_name")}
            {renderInputRow("Country Code", "country_code")}
        </Flex>
    );

    const requestTab = (
        <Flex vertical gap="middle" style={{ paddingTop: 16 }}>
            {renderInputRow("Path", "path")}
            {renderInputRow("Query", "query")}
            {renderInputRow("Method", "method")}
            {renderInputRow("User Agent", "user_agent")}
            {renderInputRow("Content Type", "content_type")}
            {renderInputRow("Accept Language", "accept_language")}
            {renderInputRow("X-Request-ID", "x_request_id")}
        </Flex>
    );

    const tabItems: TabsProps["items"] = [
        {
            key: "general",
            label: t("General"),
            children: genTab,
        },
        {
            key: "network",
            label: <span><CloudServerOutlined /> {t("Network")}</span>,
            children: networkTab,
        },
        {
            key: "location",
            label: <span><GlobalOutlined /> {t("Location")}</span>,
            children: locationTab,
        },
        {
            key: "request",
            label: <span><FileSearchOutlined /> {t("Request")}</span>,
            children: requestTab,
        },
    ];

    // --- Render ---

    if (isDelete) {
        return (
            <Modal
                title={title}
                open
                onOk={handleOk}
                onCancel={handleCancel}
                okText={t("Ok")}
                cancelText={t("Cancel")}
                confirmLoading={loading}
                width={640}
            >
                {error && (
                    <Alert
                        message={error}
                        type="error"
                        showIcon
                        closable
                        onClose={() => setError(null)}
                        style={{ marginBottom: 16 }}
                    />
                )}
                <Text>{confirmMessage}</Text>
            </Modal>
        );
    }

    if (isFormMode) {
        return (
            <Modal
                title={title}
                open
                onOk={handleOk}
                onCancel={handleCancel}
                okText={t("Ok")}
                cancelText={t("Cancel")}
                confirmLoading={loading}
                width={640}
                styles={{ body: { maxHeight: "60vh", overflowY: "auto" } }}
            >
                {error && (
                    <Alert
                        message={error}
                        type="error"
                        showIcon
                        closable
                        onClose={() => setError(null)}
                        style={{ marginBottom: 16 }}
                    />
                )}
                <Tabs defaultActiveKey="general" items={tabItems} size="middle" />
            </Modal>
        );
    }

    return null;
}