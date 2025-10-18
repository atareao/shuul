import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Modal, Flex, Typography, Input, InputNumber, Switch } from "antd";

const { Text } = Typography;

import type Rule from "@/models/rule";
import { BASE_URL } from '@/constants';

interface State {
    rule?: Rule;
    isOpen: boolean;
}

interface Props {
    rule?: Rule;
    isOpen?: boolean;
    navigate: any;
    t: any;
    onClose: () => void;
}

class InnerDialog extends React.Component<Props, State> {
    constructor(props: Props) {
        super(props);
        this.state = {
            rule: this.props.rule !== undefined ? this.props.rule : this.fields.reduce((acc: any, field: any) => {
                acc[field.key] = field.default;
                return acc
            }, {}),
            isOpen: this.props.isOpen !== undefined ? this.props.isOpen : false,
        }
    }

    getValue = (key: string) => {
        const keyName = key as keyof Rule;
        if (this.state.rule) {
            return this.state.rule[keyName];
        }
        return this.state.rule ? this.state.rule[keyName] : this.fields.find(field => field.key === key)?.default;
    }

    fields = [
        { key: 'active', label: this.props.t('Active'), type: 'switch', default: true },
        { key: 'allow', label: this.props.t('Allow'), type: 'switch', default: false },
        { key: 'weight', label: this.props.t('Weight'), type: 'inputnumber', default: 100 },
        { key: 'ip_address', label: this.props.t('IP Address'), type: 'input', default: "" },
        { key: 'protocol', label: this.props.t('Protocol'), type: 'input', default: "" },
        { key: 'fqdn', label: this.props.t('FQDN'), type: 'input', default: "" },
        { key: 'path', label: this.props.t('Path'), type: 'input', default: "" },
        { key: 'query', label: this.props.t('Query'), type: 'input', default: "" },
        { key: 'city_name', label: this.props.t('City Name'), type: 'input', default: "" },
        { key: 'country_name', label: this.props.t('Contry Name'), type: 'input', default: "" },
        { key: 'couunty_code', label: this.props.t('Contry Code'), type: 'input', default: "" },
    ]

    onChange = (key: string, value: any) => {
        this.setState((prevState) => ({
            rule: {
                ...prevState.rule,
                [key]: value,
            } as Rule,
        })
        );
    }

    fetchData = async () => {
        console.log("fetch data");
        const isNew = this.props.rule === undefined;
        console.log("Is new rule:", isNew);
        const url = new URL(`${BASE_URL}/api/v1/rules`).toString();
        console.log("Request URL:", url);
        const body = this.fields.reduce((acc: any, field: any) => {
            acc[field.key] = this.getValue(field.key);
            return acc;
        }, {})
        console.log("Initial request body:", body);
        if (isNew) {
            body["id"] = this.state.rule?.id;
        }
        const string_body = JSON.stringify(body);
        console.log("Request body:", string_body);
        try {
            const response = await fetch(url, {
                method: isNew ? 'POST' : 'PATCH',
                headers: {
                    "Content-Type": "application/json",
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
        if (prevProps.isOpen !== this.props.isOpen) {
            this.setState({ isOpen: this.props.isOpen !== undefined ? this.props.isOpen : false });
        }
    }

    render = () => {
        const title = this.props.rule ? this.props.t('Edit Rule') : this.props.t('Add Rule');
        return (
            <Modal
                title={title}
                open={this.state.isOpen}
                onOk={async () => {
                    const response = await this.fetchData();
                    console.log("Response after fetchData:", response);
                    this.setState({ isOpen: false });
                    this.props.onClose();
                }}
                onCancel={() => {
                    this.setState({ isOpen: false });
                    this.props.onClose();
                }}
                okText={this.props.t('Ok')}
                cancelText={this.props.t('Cancel')}
            >
                <Flex vertical gap="small">
                    {this.fields && this.fields.map((field) => (
                        <Flex key={field.key}>
                            <Text style={{ width: 200 }}>{field.label}</Text>
                            {field.type === 'switch' &&
                                <Switch
                                    defaultChecked={this.getValue(field.key) as boolean}
                                    onChange={(checked) => this.onChange(field.key, checked)}
                                />
                            }
                            {field.type === 'input' &&
                                <Input
                                    style={{ width: '100%' }}
                                    defaultValue={this.getValue(field.key) as string}
                                    placeholder={field.label}
                                    onChange={(e) => this.onChange(field.key, e.target.value)}
                                />
                            }
                            {field.type === 'inputnumber' &&
                                <InputNumber
                                    style={{ width: '100%' }}
                                    defaultValue={this.getValue(field.key) as number}
                                    placeholder={field.label}
                                    onChange={(value) => this.onChange(field.key, value)}
                                />
                            }
                        </Flex>
                    ))}
                </Flex>
            </Modal>
        );
    }
}

interface RuleDialogProps {
    rule?: Rule;
    isOpen?: boolean;
    onClose: () => void;
}

export default function RuleDialog(props: RuleDialogProps) {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerDialog
        rule={props.rule}
        isOpen={props.isOpen}
        onClose={props.onClose}
        navigate={navigate}
        t={t}
    />;
}
