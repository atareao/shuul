import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Modal, Typography } from "antd";

const { Text } = Typography;

import type Rule from "@/models/rule";
import { BASE_URL } from '@/constants';

interface State {
    isOpen: boolean;
    rule: Rule,
}

interface Props {
    isOpen?: boolean;
    navigate: any;
    t: any;
    rule: Rule,
    onClose: (deleted: boolean, rule: Rule) => void;
}

class InnerDialog extends React.Component<Props, State> {
    constructor(props: Props) {
        super(props);
        this.state = {
            rule: this.props.rule,
            isOpen: this.props.isOpen !== undefined ? this.props.isOpen : false,
        }
    }

    fetchData = async () => {
        console.log("delete data");
        const url = new URL(`${BASE_URL}/api/v1/rules?id=${this.state.rule.id}`).toString();
        console.log("Request URL:", url);
        try {
            const response = await fetch(url, {
                method: 'DELETE',
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
            this.props.onClose(true, this.state.rule);
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
        if (prevProps.isOpen !== this.props.isOpen ||
                prevProps.rule.id !== this.props.rule.id) {
            this.setState({ 
                rule: {
                    ...prevProps.rule,
                    ...this.props.rule
                },
                isOpen: this.props.isOpen !== undefined ? this.props.isOpen : false
            });
        }
    }

    render = () => {
        const rule_id = this.state.rule.id;
        return (
            <Modal
                title={this.props.t('Delete Rule')}
                open={this.state.isOpen}
                onOk={async () => {
                    await this.fetchData();
                    this.props.onClose(true, this.state.rule);
                }}
                onCancel={() => {
                    this.setState({ isOpen: false });
                    this.props.onClose(false, this.state.rule);
                }}
                okText={this.props.t('Ok')}
                cancelText={this.props.t('Cancel')}
            >
                <Text>{this.props.t(`Are you sure you want to delete rule "${rule_id}"?`)}</Text>
            </Modal>
        );
    }
}

interface RuleDialogProps {
    rule: Rule,
    isOpen?: boolean;
    onClose: (deleted: boolean, rule: Rule) => void;
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
