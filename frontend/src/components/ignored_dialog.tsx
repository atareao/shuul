import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Modal, Typography, Flex, Input, InputNumber, Switch } from "antd";

const { Text } = Typography;

import type Item from "@/models/ignored";
import { BASE_URL } from '@/constants';
import type { DialogMode } from '@/common/types';
import { DialogModes } from '@/common/types';

const ENDPOINT = '/api/v1/ignored';
const MESSAGES = {
    create: "Create Ignored Rule",
    update: "Update Ignored Rule",
    read: "Read Ignored Rule",
    delete: "Delete Ignored Rule",
    confirm_delete: 'Are you sure you want to delete this Ignore Rule',
};
const FIELDS = [
    { key: 'active', label:'Active', type: 'boolean', value: true },
    { key: 'ip_address', label: 'IP Address', type: 'string', value: "" },
    { key: 'protocol', label: 'Protocol', type: 'string', value: "" },
    { key: 'fqdn', label: 'FQDN', type: 'string', value: "" },
    { key: 'path', label: 'Path', type: 'string', value: "" },
    { key: 'query', label: 'Query', type: 'string', value: "" },
    { key: 'city_name', label: 'City Name', type: 'string', value: "" },
    { key: 'country_name', label: 'Contry Name', type: 'string', value: "" },
    { key: 'country_code', label: 'Contry Code', type: 'string', value: "" },
];

interface State {
    item?: Item,
}

interface Props {
    item?: Item,
    dialogMode?: DialogMode;
    onClose: (ok: boolean, item?: Item) => void;
    navigate: any;
    t: any;
}

class InnerDialog extends React.Component<Props, State> {
    constructor(props: Props) {
        super(props);
        this.state = {
            item: this.props.item !== undefined ? this.props.item : FIELDS.reduce((acc: any, field: any) => {
                acc[field.key] = field.value;
                return acc
            }, {}),
        }
    }

    getValue = (key: string) => {
        const keyName = key as keyof Item;
        if (this.state.item) {
            return this.state.item[keyName];
        }
        return FIELDS.find(field => field.key === key)?.value;

    }

    fetchData = async () => {
        let method;
        let url;
        let string_body;
        let queryString;
        const basePath = `${BASE_URL}${ENDPOINT}`;
        const searchParams = new URLSearchParams();
        if (this.props.dialogMode === DialogModes.DELETE) {
            method = 'DELETE';
            if(this.state.item?.id !== undefined){
                searchParams.append('id', String(this.state.item.id));
                queryString = searchParams.toString();
            }
            string_body = null;
        } else if (this.props.dialogMode === DialogModes.UPDATE) {
            method = 'PATCH';
            const body = FIELDS.reduce((acc: any, field: any) => {
                acc[field.key] = this.state.item?.[field.key as keyof Item];
                return acc;
            }, {})
            body["id"] = this.state.item?.id;
            string_body = JSON.stringify(body);

        } else if (this.props.dialogMode === DialogModes.CREATE) {
            method = 'POST';
            const body = FIELDS.reduce((acc: any, field: any) => {
                acc[field.key] = this.state.item?.[field.key as keyof Item];
                return acc;
            }, {})
            string_body = JSON.stringify(body);
        } else if (this.props.dialogMode === DialogModes.READ) {
            method = 'GET';
            if(this.state.item?.id !== undefined){
                searchParams.append('id', String(this.state.item.id));
                queryString = searchParams.toString();
            }
            string_body = null;
        } else {
            return null;
        }
        url = `${basePath}?${queryString && queryString.trim() !== "" ? `?${queryString}` : ''}`;
        console.log("Request URL:", url);
        try {
            const response = await fetch(url, {
                method: method,
                headers: {
                    "Content-Type": "application/json",
                    "Accept": "application/json",
                },
                body: string_body,
            })
            if (!response.ok) {
                let errorBody: { message?: string } = {};
                try {
                    // Intentar leer el cuerpo del error si está en formato JSON
                    errorBody = await response.json();
                } catch (e) {
                    // Si falla al parsear, ignorar y usar un mensaje por defecto
                }
                return {
                    status: response.status,
                    message: errorBody.message || `Error HTTP: ${response.status} - ${response.statusText}`
                };
            }
            const content = await response.json();
            console.log("Fetch successful:", content);
            this.props.onClose(true, content.data);
        } catch (error) {
            const msg = error instanceof Error ? error.message : String(error);
            console.error('Network Error or Fetch Failure:', msg, error);

            return {
                status: 500,
                message: `Network or Unknown Error: ${msg}`
            };
        }
    }

    componentDidUpdate(prevProps: Props) {
        if (prevProps.dialogMode !== this.props.dialogMode ||
            (this.props.dialogMode !== DialogModes.CREATE &&
                prevProps.item?.id !== this.props.item?.id)) {
            if (this.props.item && this.props.dialogMode !== DialogModes.CREATE) {
                this.setState({
                    item: {
                        ...prevProps.item,
                        ...this.props.item
                    },
                });
            }
        }
    }

    onChange = (key: string, value: any) => {
        this.setState((prevState) => ({
            item: {
                ...prevState.item,
                [key]: value,
            } as Item,
        })
        );
    }

    render = () => {
        const dialogMode = this.props.dialogMode;
        const item_id = this.state.item?.id;
        const disabled = dialogMode === DialogModes.READ;
        let title = "";
        let message = "";
        if (dialogMode === DialogModes.CREATE) {
            title = this.props.t(MESSAGES.create);
        }else if (dialogMode === DialogModes.UPDATE) {
            title = this.props.t(MESSAGES.update);
        }else if (dialogMode === DialogModes.READ) {
            title = this.props.t(MESSAGES.read);
        }else if (dialogMode === DialogModes.DELETE) {
            title = this.props.t(MESSAGES.delete);
            message = this.props.t(`${MESSAGES.confirm_delete} ${item_id}?`);
        }
        return (
            <>
                {(dialogMode === DialogModes.DELETE) &&
                    <Modal
                        title={title}
                        open={this.props.dialogMode !== undefined}
                        onOk={async () => {
                            await this.fetchData();
                            this.props.onClose(true, this.state.item);
                        }}
                        onCancel={() => {
                            this.props.onClose(false, this.state.item);
                        }}
                        okText={this.props.t('Ok')}
                        cancelText={this.props.t('Cancel')}
                    >
                        <Text>{message}</Text>
                    </Modal>
                }
                {(dialogMode === DialogModes.CREATE || dialogMode === DialogModes.UPDATE) &&
                    <Modal
                        title={title}
                        open={this.props.dialogMode !== undefined}
                        onOk={async () => {
                            await this.fetchData();
                        }}
                        onCancel={() => {
                            this.props.onClose(false);
                        }}
                        okText={this.props.t('Ok')}
                        cancelText={this.props.t('Cancel')}
                    >
                        <Flex vertical gap="small">
                            {FIELDS.map((field) => (
                                <Flex key={field.key}>
                                    <Text style={{ width: 200 }}>{this.props.t(field.label)}</Text>
                                    {field.type === 'boolean' &&
                                        <Switch
                                            defaultChecked={this.getValue(field.key) as boolean}
                                            onChange={(checked) => this.onChange(field.key, checked)}
                                            disabled={disabled}
                                        />
                                    }
                                    {field.type === 'string' &&
                                        <Input
                                            style={{ width: '100%' }}
                                            defaultValue={this.getValue(field.key) as string}
                                            placeholder={this.props.t(field.label)}
                                            onChange={(e) => this.onChange(field.key, e.target.value)}
                                            disabled={disabled}
                                        />
                                    }
                                    {field.type === 'number' &&
                                        <InputNumber
                                            style={{ width: '100%' }}
                                            defaultValue={this.getValue(field.key) as number}
                                            placeholder={this.props.t(field.label)}
                                            onChange={(value) => this.onChange(field.key, value)}
                                            disabled={disabled}
                                        />
                                    }
                                </Flex>
                            ))}
                        </Flex>
                    </Modal>
                }
            </>
        );
    }
}

interface DialogProps {
    item?: Item,
    dialogMode?: DialogMode;
    onClose: (ok: boolean, item?: Item) => void;
}

export default function Dialog(props: DialogProps) {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerDialog
        item={props.item}
        dialogMode={props.dialogMode}
        onClose={props.onClose}
        navigate={navigate}
        t={t}
    />;
}
