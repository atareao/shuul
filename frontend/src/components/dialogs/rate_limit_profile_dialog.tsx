import { useState, useEffect, useCallback } from "react";
import { Modal, Typography, Flex, Input, InputNumber, Switch, Alert, Tabs } from "antd";
import type { TabsProps } from "antd";
import { ThunderboltOutlined } from '@ant-design/icons';
import { BASE_URL } from '@/constants';
import type { DialogMode } from '@/common/types';
import { DialogModes } from '@/common/types';
import type { DialogMessages } from '@/components/dialogs/custom_dialog';
import type Item from "@/models/rate_limit_profile";

const { Text } = Typography;

// Props del RateLimitProfileDialog
interface RateLimitProfileDialogProps {
    dialogMode: DialogMode;
    selectedItem: Item | undefined;
    handleCloseDialog: (item?: Item | undefined) => void;
    endpoint: string;
    dialogMessages?: DialogMessages;
    t: (key: string) => string;
}

// Valores por defecto para CREATE mode
const DEFAULT_VALUES: Record<string, any> = {
    name: "",
    description: "",
    max_retry: 5,
    find_time_seconds: 600,
    ban_time_seconds: 3600,
    bantime_increment: false,
    bantime_multipliers: "",
    bantime_maxtime_seconds: 604800,
    ban_count_decay_days: 30,
    fail_codes: "401,403,404",
};

// Inicializa los valores del formulario desde un Item o desde valores por defecto
function initializeFromItem(item?: Item): Record<string, any> {
    if (!item) {
        return { ...DEFAULT_VALUES };
    }
    return {
        name: item.name ?? "",
        description: item.description ?? "",
        max_retry: item.max_retry ?? 5,
        find_time_seconds: item.find_time_seconds ?? 600,
        ban_time_seconds: item.ban_time_seconds ?? 3600,
        bantime_increment: item.bantime_increment ?? false,
        bantime_multipliers: item.bantime_multipliers
            ? item.bantime_multipliers.join(",")
            : "",
        bantime_maxtime_seconds: item.bantime_maxtime_seconds ?? 604800,
        ban_count_decay_days: item.ban_count_decay_days ?? 30,
        fail_codes: item.fail_codes ? item.fail_codes.join(",") : "401,403,404",
    };
}

// Convierte los valores del formulario al formato esperado por la API
function formatForApi(values: Record<string, any>): Record<string, any> {
    const body: Record<string, any> = {
        name: values.name || null,
        description: values.description || null,
        max_retry: values.max_retry,
        find_time_seconds: values.find_time_seconds,
        ban_time_seconds: values.ban_time_seconds,
        bantime_increment: values.bantime_increment,
        bantime_multipliers: values.bantime_multipliers
            ? values.bantime_multipliers.split(",").map((s: string) => Number(s.trim())).filter((n: number) => !isNaN(n))
            : [],
        bantime_maxtime_seconds: values.bantime_maxtime_seconds,
        ban_count_decay_days: values.ban_count_decay_days,
        fail_codes: values.fail_codes
            ? values.fail_codes.split(",").map((s: string) => Number(s.trim())).filter((n: number) => !isNaN(n))
            : [],
    };
    return body;
}

export default function RateLimitProfileDialog({
    dialogMode,
    selectedItem,
    handleCloseDialog,
    endpoint,
    dialogMessages,
    t,
}: RateLimitProfileDialogProps) {
    const [formValues, setFormValues] = useState<Record<string, any>>(() =>
        initializeFromItem(selectedItem)
    );
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

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

    let title = "";
    let confirmMessage = "";

    if (isCreate) {
        title = t(dialogMessages?.createTitle || "Create Rate Limit Profile");
    } else if (isUpdate) {
        title = t(dialogMessages?.updateTitle || "Update Rate Limit Profile");
    } else if (dialogMode === DialogModes.READ) {
        title = t(dialogMessages?.readTitle || "View Rate Limit Profile");
    } else if (isDelete) {
        title = t(dialogMessages?.deleteTitle || "Delete Rate Limit Profile");
        confirmMessage = dialogMessages?.confirmDeleteMessage
            ? t(dialogMessages.confirmDeleteMessage(selectedItem?.id ?? ""))
            : t("Are you sure you want to delete this rate limit profile?");
    }

    // --- Render helpers ---

    const renderSwitchRow = (label: string, key: string) => (
        <Flex align="center" gap="small">
            <Text style={{ width: 140, flexShrink: 0 }}>{t(label)}</Text>
            <Switch
                checked={Boolean(formValues[key])}
                onChange={(checked) => updateField(key, checked)}
                disabled={disabled}
            />
        </Flex>
    );

    const renderInputRow = (label: string, key: string, placeholder?: string) => (
        <Flex align="center" gap="small">
            <Text style={{ width: 140, flexShrink: 0 }}>{t(label)}</Text>
            <Input
                style={{ width: "100%" }}
                value={formValues[key] ?? ""}
                placeholder={placeholder || t(label)}
                onChange={(e) => updateField(key, e.target.value)}
                disabled={disabled}
            />
        </Flex>
    );

    const renderInputNumberRow = (label: string, key: string, min?: number, max?: number) => (
        <Flex align="center" gap="small">
            <Text style={{ width: 140, flexShrink: 0 }}>{t(label)}</Text>
            <InputNumber
                style={{ width: 200 }}
                value={formValues[key] as number}
                min={min}
                max={max}
                onChange={(value) => updateField(key, value ?? 0)}
                disabled={disabled}
            />
        </Flex>
    );

    // --- Tab contents ---

    const generalTab = (
        <Flex vertical gap="middle" style={{ paddingTop: 16 }}>
            {renderInputRow("Name", "name")}
            {renderInputRow("Description", "description")}
            {renderInputNumberRow("Max Retry", "max_retry", 1, 99999)}
            {renderInputNumberRow("Find Time (s)", "find_time_seconds", 1, 999999)}
            {renderInputRow("Fail Codes", "fail_codes", "e.g. 401,403,404")}
        </Flex>
    );

    const penaltyTab = (
        <Flex vertical gap="middle" style={{ paddingTop: 16 }}>
            {renderInputNumberRow("Ban Time (s)", "ban_time_seconds", 1, 999999)}
            {renderInputNumberRow("Max Ban (s)", "bantime_maxtime_seconds", 1, 9999999)}
            {renderInputNumberRow("Decay (d)", "ban_count_decay_days", 1, 9999)}
            {renderSwitchRow("Escalate", "bantime_increment")}
            {renderInputRow("Multipliers", "bantime_multipliers", "e.g. 1,2,4,8")}
        </Flex>
    );

    const tabItems: TabsProps["items"] = [
        {
            key: "general",
            label: t("General"),
            children: generalTab,
        },
        {
            key: "penalty",
            label: <span><ThunderboltOutlined /> {t("Penalty")}</span>,
            children: penaltyTab,
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