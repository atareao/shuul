import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Modal, Typography, Flex, Input, InputNumber, Switch } from "antd";

const { Text } = Typography;

import type Rule from "@/models/rule";
import { BASE_URL } from '@/constants';
import type { DialogMode } from '@/common/types';
import { DialogModes } from '@/common/types';



interface State {
    rule?: Rule,
}

interface Props {
    rule?: Rule,
    dialogMode?: DialogMode;
    onClose: (ok: boolean, rule?: Rule) => void;
    navigate: any;
    t: any;
}

class InnerDialog extends React.Component<Props, State> {
    constructor(props: Props) {
        super(props);
        this.state = {
            rule: this.props.rule !== undefined ? this.props.rule : this.fields.reduce((acc: any, field: any) => {
                acc[field.key] = field.value;
                return acc
            }, {}),
        }
    }

    fields = [
        { key: 'active', label: this.props.t('Active'), type: 'boolean', value: true },
        { key: 'allow', label: this.props.t('Allow'), type: 'boolean', value: false },
        { key: 'weight', label: this.props.t('Weight'), type: 'number', value: 100 },
        { key: 'ip_address', label: this.props.t('IP Address'), type: 'string', value: "" },
        { key: 'protocol', label: this.props.t('Protocol'), type: 'string', value: "" },
        { key: 'fqdn', label: this.props.t('FQDN'), type: 'string', value: "" },
        { key: 'path', label: this.props.t('Path'), type: 'string', value: "" },
        { key: 'query', label: this.props.t('Query'), type: 'string', value: "" },
        { key: 'city_name', label: this.props.t('City Name'), type: 'string', value: "" },
        { key: 'country_name', label: this.props.t('Contry Name'), type: 'string', value: "" },
        { key: 'country_code', label: this.props.t('Contry Code'), type: 'string', value: "" },
    ]


    getValue = (key: string) => {
        const keyName = key as keyof Rule;
        if (this.state.rule) {
            return this.state.rule[keyName];
        }
        return this.fields.find(field => field.key === key)?.value;

    }

    fetchData = async () => {
        let method;
        let url;
        let string_body;
        let queryString;
        const basePath = `${BASE_URL}/api/v1/rules`;
        const searchParams = new URLSearchParams();
        if (this.props.dialogMode === DialogModes.DELETE) {
            method = 'DELETE';
            if(this.state.rule?.id !== undefined){
                searchParams.append('id', String(this.state.rule.id));
                queryString = searchParams.toString();
            }
            string_body = null;
        } else if (this.props.dialogMode === DialogModes.UPDATE) {
            method = 'PATCH';
            const body = this.fields.reduce((acc: any, field: any) => {
                acc[field.key] = this.state.rule?.[field.key as keyof Rule];
                return acc;
            }, {})
            body["id"] = this.state.rule?.id;
            string_body = JSON.stringify(body);

        } else if (this.props.dialogMode === DialogModes.CREATE) {
            method = 'POST';
            const body = this.fields.reduce((acc: any, field: any) => {
                acc[field.key] = this.state.rule?.[field.key as keyof Rule];
                return acc;
            }, {})
            string_body = JSON.stringify(body);
        } else if (this.props.dialogMode === DialogModes.READ) {
            method = 'GET';
            if(this.state.rule?.id !== undefined){
                searchParams.append('id', String(this.state.rule.id));
                queryString = searchParams.toString();
            }
            string_body = null;
        } else {
            return null;
        }
        url = `${basePath}?${queryString ? `?${queryString}` : ''}`;
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
                prevProps.rule?.id !== this.props.rule?.id)) {
            if (this.props.rule && this.props.dialogMode !== DialogModes.CREATE) {
                this.setState({
                    rule: {
                        ...prevProps.rule,
                        ...this.props.rule
                    },
                });
            }
        }
    }

    onChange = (key: string, value: any) => {
        this.setState((prevState) => ({
            rule: {
                ...prevState.rule,
                [key]: value,
            } as Rule,
        })
        );
    }

    render = () => {
        const dialogMode = this.props.dialogMode;
        const rule_id = this.state.rule?.id;
        const disabled = dialogMode === DialogModes.READ;
        let title = "";
        let message = "";
        if (dialogMode === DialogModes.CREATE) {
            title = this.props.t('Create Rule');
        }else if (dialogMode === DialogModes.UPDATE) {
            title = this.props.t('Update Rule');
        }else if (dialogMode === DialogModes.READ) {
            title = this.props.t('View Rule');
        }else if (dialogMode === DialogModes.DELETE) {
            title = this.props.t('Delete Rule');
            message = this.props.t(`Are you sure you want to delete rule "${rule_id}"?`);
        }
        return (
            <>
                {(dialogMode === DialogModes.DELETE) &&
                    <Modal
                        title={title}
                        open={this.props.dialogMode !== undefined}
                        onOk={async () => {
                            await this.fetchData();
                            this.props.onClose(true, this.state.rule);
                        }}
                        onCancel={() => {
                            this.props.onClose(false, this.state.rule);
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
                            {this.fields && this.fields.map((field) => (
                                <Flex key={field.key}>
                                    <Text style={{ width: 200 }}>{field.label}</Text>
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
                                            placeholder={field.label}
                                            onChange={(e) => this.onChange(field.key, e.target.value)}
                                            disabled={disabled}
                                        />
                                    }
                                    {field.type === 'number' &&
                                        <InputNumber
                                            style={{ width: '100%' }}
                                            defaultValue={this.getValue(field.key) as number}
                                            placeholder={field.label}
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

interface RuleDialogProps {
    rule?: Rule,
    dialogMode?: DialogMode;
    onClose: (ok: boolean, rule?: Rule) => void;
}

export default function RuleDialog(props: RuleDialogProps) {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerDialog
        rule={props.rule}
        dialogMode={props.dialogMode}
        onClose={props.onClose}
        navigate={navigate}
        t={t}
    />;
}
