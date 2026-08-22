import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Modal, Typography, Flex, Input, InputNumber, Switch, Select, Alert, Tabs } from "antd";

const { Text } = Typography;


type WithOptionalId<T> = T & { id?: number };

import { BASE_URL } from '@/constants';
import type { DialogMode, FieldDefinition } from '@/common/types';
import { DialogModes } from '@/common/types';
import { getNestedValue, debounce } from "@/common/utils";

// Interfaces de State y Props, ahora genéricas en T
interface State<T> {
    data?: WithOptionalId<T>, // Renombrado de 'rule' a 'data'
    showMessage?: boolean
    messageText?: string
    messageType?: 'success' | 'error' | 'info' | 'warning'
}

interface Props<T> {
    endpoint: string;
    dialogMessages?: DialogMessages;
    data?: WithOptionalId<T>, // Renombrado de 'rule' a 'data'
    fields: FieldDefinition<T>[]; // Se añaden los campos como prop
    dialogMode?: DialogMode;
    // La función onClose ahora devuelve un tipo T
    onClose: (data?: WithOptionalId<T>) => void;
    navigate: any;
    t: any;
}

const isValidIpv4Address = (value: string): boolean => {
    const parts = value.split('.');
    return parts.length === 4 && parts.every((part) => /^\d{1,3}$/.test(part) && Number(part) <= 255);
};

const isValidIpv6Address = (value: string): boolean => {
    if (!/^[0-9a-fA-F:]+$/.test(value) || value.includes(':::')) {
        return false;
    }

    const hasCompression = value.includes('::');
    if (hasCompression && value.indexOf('::') !== value.lastIndexOf('::')) {
        return false;
    }

    const parts = value.split(':').filter(Boolean);
    if (!parts.every((part) => /^[0-9a-fA-F]{1,4}$/.test(part))) {
        return false;
    }

    return hasCompression ? parts.length < 8 : parts.length === 8;
};

const isValidIpAddress = (value: string): boolean =>
    isValidIpv4Address(value) || isValidIpv6Address(value);

// El componente de clase se hace genérico: InnerDialog<T>
class InnerDialog<T> extends React.Component<Props<T>, State<T>> {

    hideMessage: () => void;

    constructor(props: Props<T>) {
        super(props);

        // Inicialización del estado con 'data' (antes 'rule')
        console.log(props.data);
        this.state = {
            data: this.props.data !== undefined ? this.props.data : this.props.fields.reduce((acc: any, field: any) => {
                acc[field.key] = field.value;
                return acc
            }, {} as WithOptionalId<T>),
        }
        this.hideMessage = debounce(() => {
            this.setState({ showMessage: false });
        }, 3000);
    }

    // Los 'fields' ya no son una propiedad de clase fija, sino que se acceden desde this.props.fields
    // Se han eliminado los 'fields' estáticos de la clase.

    getValue = (key: keyof T & string) => {
        if (this.state.data) {
            return this.state.data[key];
        }
        // Si no hay datos, buscar el valor inicial en las props.fields
        return this.props.fields.find(field => field.key === key)?.value;
    }

    fetchData = async () => {
        let method;
        let url;
        let string_body;
        let queryString = "";
        const basePath = `${BASE_URL}/api/v1/${this.props.endpoint}`;
        const searchParams = new URLSearchParams();

        const dataWithId = this.state.data as WithOptionalId<T>; // Para facilitar el acceso al id

        if (this.props.dialogMode === DialogModes.DELETE) {
            method = 'DELETE';
            if (dataWithId?.id !== undefined) {
                searchParams.append('id', String(dataWithId.id));
                queryString = searchParams.toString();
            }
            string_body = null;
        } else if (this.props.dialogMode === DialogModes.CREATE) {
            method = 'POST';
            const body = this.props.fields.reduce((acc: any, field: FieldDefinition<T>) => {
                acc[field.key] = this.serializeFieldValue(
                    field,
                    getNestedValue(dataWithId, field.key as string),
                );
                return acc;
            }, {})
            string_body = JSON.stringify(body);
        } else if (this.props.dialogMode === DialogModes.UPDATE) {
            method = 'PATCH';
            const body = this.props.fields.reduce((acc: any, field: FieldDefinition<T>) => {
                acc[field.key] = this.serializeFieldValue(
                    field,
                    getNestedValue(dataWithId, field.key as string),
                );
                return acc;
            }, {})
            string_body = JSON.stringify(body);
        } else if (this.props.dialogMode === DialogModes.READ) {
            method = 'GET';
            if (dataWithId?.id !== undefined) {
                searchParams.append('id', String(dataWithId.id));
                queryString = searchParams.toString();
            }
            string_body = null;
        } else {
            return null;
        }

        // Manejo de URL: se usa '?' solo si queryString existe para evitar dobles '?'
        url = `${basePath}${queryString.trim() !== "" ? `?${queryString}` : ''}`;

        console.log("Request URL:", url);
        console.log("Body:", string_body);
        try {
            const response = await fetch(url, {
                method: method,
                headers: {
                    "Content-Type": "application/json",
                    "Accept": "application/json",
                },
                body: string_body,
            })
            console.log("====================");
            console.log("Response Status:", response.status);
            console.log("====================");
            if (!response.ok || response.status > 299) {
                // ... (lógica de manejo de errores, sin cambios)
                let errorBody: { message?: string } = {};
                try {
                    errorBody = await response.json();
                } catch (e) { }
                console.log(JSON.stringify(errorBody));
                return {
                    status: response.status,
                    message: errorBody.message || `Error HTTP: ${response.status} - ${response.statusText}`
                };
            }
            const content = await response.json();
            console.log("Fetch successful:", content);
            // Pasar el objeto completo (content.data) al onClose
            this.props.onClose(content.data as WithOptionalId<T>);
        } catch (error) {
            // ... (lógica de manejo de errores de red, sin cambios)
            const msg = error instanceof Error ? error.message : String(error);
            console.error('Network Error or Fetch Failure:', msg, error);

            return {
                status: 500,
                message: `Network or Unknown Error: ${msg}`
            };
        }
    }

    serializeFieldValue = (field: FieldDefinition<T>, value: any) => {
        if (field.key === 'bantime_multipliers') {
            if (Array.isArray(value)) {
                return value.map((item) => Number(item)).filter((item) => !Number.isNaN(item));
            }
            return String(value ?? "")
                .split(",")
                .map((item) => Number(item.trim()))
                .filter((item) => !Number.isNaN(item));
        }
        if (field.key === 'ignoreip') {
            if (Array.isArray(value)) {
                return value.map((item) => String(item).trim()).filter(Boolean);
            }
            return String(value ?? "")
                .split(",")
                .map((item) => item.trim())
                .filter(Boolean);
        }
        if (typeof value === 'string' && value.trim() === "") {
            return null;
        }
        return value;
    }

    componentDidUpdate(prevProps: Props<T>) {
        if (prevProps.dialogMode !== this.props.dialogMode ||
            (this.props.dialogMode !== DialogModes.CREATE &&
                prevProps.data?.id !== this.props.data?.id)) { // Renombrado de 'rule' a 'data'
            if (this.props.data && this.props.dialogMode !== DialogModes.CREATE) {
                this.setState({
                    data: { // Renombrado de 'rule' a 'data'
                        ...prevProps.data,
                        ...this.props.data
                    },
                });
            }
        }
    }

    handleClose = async (ok: boolean) => {
        console.log("Handling close, ok =", ok);
        if (ok) {
            if (this.state.data === undefined) {
                const requiredFields = this.props.fields.filter(field => field.required).map(field => field.label).join(", ");
                this.showMessage(`Los siguientes campos son obligatorios: ${requiredFields}`, "error");
                return;
            }
            for (const field of this.props.fields) {
                console.log(field);
                console.log(getNestedValue(this.state.data, field.key as string));
                const fieldValue = getNestedValue(this.state.data, field.key as string);
                if (field.required) {
                    if (fieldValue === undefined || fieldValue === null || fieldValue === '') {
                        this.showMessage(`El campo ${field.label} es obligatorio`, "error");
                        return;
                    }
                }
                if (field.type === 'number' && typeof fieldValue === 'number') {
                    if (field.min !== undefined && fieldValue < field.min) {
                        this.showMessage(`El campo ${field.label} debe ser mayor o igual a ${field.min}`, "error");
                        return;
                    }
                    if (field.max !== undefined && fieldValue > field.max) {
                        this.showMessage(`El campo ${field.label} debe ser menor o igual a ${field.max}`, "error");
                        return;
                    }
                }
                if (field.key === 'ip_address' && typeof fieldValue === 'string' && fieldValue.trim() !== '') {
                    if (!isValidIpAddress(fieldValue.trim())) {
                        this.showMessage(`La dirección IP no es válida: ${fieldValue}`, "error");
                        return;
                    }
                }
                if (field.key === 'ignoreip') {
                    const ignoreIps = this.serializeFieldValue(field, fieldValue) as string[];
                    const invalidIp = ignoreIps.find((ip) => !isValidIpAddress(ip));
                    if (invalidIp) {
                        this.showMessage(`La IP ignorada no es válida: ${invalidIp}`, "error");
                        return;
                    }
                }
                if (field.key === 'webhook' && typeof fieldValue === 'string' && fieldValue.trim() !== '') {
                    try {
                        const url = new URL(fieldValue);
                        if (!['http:', 'https:'].includes(url.protocol)) {
                            throw new Error("invalid protocol");
                        }
                    } catch {
                        this.showMessage(`La URL del webhook no es válida: ${fieldValue}`, "error");
                        return;
                    }
                }
            }
            const response = await this.fetchData();
            console.log(response);
            this.showMessage(response?.message || "Operación realizada con éxito", response && response.status && response.status >= 200 && response.status < 300 ? "success" : "error");
        }else{
            this.props.onClose(undefined); // Renombrado de 'rule' a 'data'
        }
    }

    // La clave ahora está tipada como keyof T & string
    onChange = (key: keyof T & string, value: any) => {
        this.setState((prevState) => ({
            data: { // Renombrado de 'rule' a 'data'
                ...prevState.data,
                [key]: value,
            } as WithOptionalId<T>,
        })
        );
    }

    showMessage = (text: string, type: 'success' | 'error' | 'info' | 'warning') => {
        this.setState({
            showMessage: true,
            messageText: text,
            messageType: type
        });
        this.hideMessage();
    }

    getVisibleFields = () => {
        const rateLimitEnabled = Boolean(getNestedValue(this.state.data, 'rate_limit_enabled'));
        const bantimeIncrement = Boolean(getNestedValue(this.state.data, 'bantime_increment'));
        const rateLimitOnlyFields = ['max_retry', 'find_time_seconds', 'ban_time_seconds', 'bantime_increment', 'ignoreip', 'webhook'];
        const escalationOnlyFields = ['bantime_multipliers', 'bantime_maxtime_seconds', 'ban_count_decay_days'];

        return this.props.fields.filter((field) => {
            if (!field.visible) {
                return false;
            }
            if (rateLimitOnlyFields.includes(field.key) && !rateLimitEnabled) {
                return false;
            }
            if (escalationOnlyFields.includes(field.key) && (!rateLimitEnabled || !bantimeIncrement)) {
                return false;
            }
            return true;
        });
    }

    renderField = (field: FieldDefinition<T>, disabled: boolean) => (
        <Flex key={field.key}>
            <Flex vertical style={{ width: 200 }}>
                <Text>{field.label}</Text>
                {field.help && <Text type="secondary">{field.help}</Text>}
            </Flex>
            {field.type === 'boolean' &&
                <Switch
                    defaultChecked={this.getValue(field.key as keyof T & string) as boolean}
                    onChange={(checked) => this.onChange(field.key as keyof T & string, checked)}
                    disabled={disabled}
                />
            }
            {field.type === 'string' &&
                <Input
                    style={{ width: '100%' }}
                    defaultValue={this.getValue(field.key as keyof T & string) as string}
                    placeholder={field.label}
                    onChange={(e) => this.onChange(field.key as keyof T & string, e.target.value)}
                    disabled={disabled || field.editable === false}
                />
            }
            {field.type === 'number' &&
                <InputNumber
                    style={{ width: '100%' }}
                    defaultValue={this.getValue(field.key as keyof T & string) as number}
                    placeholder={field.label}
                    onChange={(value) => this.onChange(field.key as keyof T & string, value)}
                    min={field.min}
                    max={field.max}
                    disabled={disabled || field.editable === false}
                />
            }
            {field.type === 'select' &&
                <Select
                    style={{ width: '100%' }}
                    defaultValue={this.getValue(field.key as keyof T & string) as any}
                    onChange={(value) => this.onChange(field.key as keyof T & string, value)}
                    disabled={disabled}
                    options={field.options}
                />
            }
        </Flex>
    )

    renderFormFields = (disabled: boolean) => {
        const fields = this.getVisibleFields();
        const tabs = Array.from(new Set(fields.map((field) => field.dialogTab).filter(Boolean)));

        if (tabs.length <= 1) {
            return (
                <Flex vertical gap="small">
                    {fields.map((field) => this.renderField(field, disabled))}
                </Flex>
            );
        }

        return (
            <Tabs
                items={tabs.map((tab) => ({
                    key: tab!,
                    label: tab,
                    children: (
                        <Flex vertical gap="small">
                            {fields
                                .filter((field) => field.dialogTab === tab)
                                .map((field) => this.renderField(field, disabled))}
                        </Flex>
                    ),
                }))}
            />
        );
    }

    render = () => {
        const { showMessage, messageText, messageType } = this.state;
        const dialogMode = this.props.dialogMode;
        // Obtener la clave 'id' de forma segura para usar en el mensaje de borrado.
        const data_id = this.state.data ? (this.state.data as any).id : undefined;
        const disabled = dialogMode === DialogModes.READ;
        let title = "";
        let message = "";
        if (dialogMode === DialogModes.CREATE) {
            title = this.props.t(this.props.dialogMessages?.createTitle); // Título genérico
        } else if (dialogMode === DialogModes.UPDATE) {
            title = this.props.t(this.props.dialogMessages?.updateTitle); // Título genérico
        } else if (dialogMode === DialogModes.READ) {
            title = this.props.t(this.props.dialogMessages?.readTitle); // Título genérico
        } else if (dialogMode === DialogModes.DELETE) {
            title = this.props.t(this.props.dialogMessages?.deleteTitle); // Título genérico
            message = this.props.t(this.props.dialogMessages?.confirmDeleteMessage(data_id)); // Mensaje genérico
        }
        return (
            <>
                {/* Alerta de campos obligatorios */}
                {(dialogMode === DialogModes.DELETE) &&
                    <Modal
                        title={title}
                        open={this.props.dialogMode !== undefined}
                        onOk={async () => {
                            await this.handleClose(true);
                        }}
                        onCancel={async () => {
                            await this.handleClose(false);
                        }}
                        okText={this.props.t('Ok')}
                        cancelText={this.props.t('Cancel')}
                    >
                        {showMessage && (
                            <Alert
                                message={messageText}
                                type={messageType}
                                showIcon
                                closable
                                onClose={() => this.setState({ showMessage: false })}
                                style={{ margin: 16 }}
                            />
                        )}
                        <Text>{message}</Text>
                    </Modal>
                }
                {(dialogMode === DialogModes.CREATE || dialogMode === DialogModes.UPDATE) &&
                    <Modal
                        title={title}
                        open={this.props.dialogMode !== undefined}
                        onOk={async () => {
                            await this.handleClose(true);
                        }}
                        onCancel={async () => {
                            await this.handleClose(false);
                        }}
                        okText={this.props.t('Ok')}
                        cancelText={this.props.t('Cancel')}
                        width={900}
                        styles={{ body: { maxHeight: '70vh', overflowY: 'auto' } }}
                    >
                        {showMessage && (
                            <Alert
                                message={messageText}
                                type={messageType}
                                showIcon
                                closable
                                onClose={() => this.setState({ showMessage: false })}
                                style={{ margin: 16 }}
                            />
                        )}
                        {this.renderFormFields(disabled)}
                    </Modal>
                }
            </>
        );
    }
}

export interface DialogMessages {
    createTitle: string;
    readTitle: string;
    updateTitle: string;
    deleteTitle: string;
    confirmDeleteMessage: (id: number | string) => string;
}

// Interfaz para el componente funcional que se exporta, ahora genérica en T
export interface DialogProps<T> {
    endpoint: string;
    dialogMessages?: DialogMessages;
    data?: WithOptionalId<T>,
    fields: FieldDefinition<T>[];
    dialogMode?: DialogMode;
    onClose: (data?: WithOptionalId<T>) => void; // Tipo T
}

export default function CustomDialog<T>(props: DialogProps<T>) {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerDialog<T>
        endpoint={props.endpoint}
        dialogMessages={props.dialogMessages}
        data={props.data}
        fields={props.fields}
        dialogMode={props.dialogMode}
        onClose={props.onClose}
        navigate={navigate}
        t={t}
    />;
}
