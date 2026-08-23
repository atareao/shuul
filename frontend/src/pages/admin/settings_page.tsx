import React from 'react';
import { Card, Form, InputNumber, Button, Typography, message, Flex } from 'antd';
import { BASE_URL } from '@/constants';

const { Title } = Typography;

interface Settings {
    log_retention_days: number;
}

interface State {
    settings: Settings | null;
    loading: boolean;
    saving: boolean;
}

export default class SettingsPage extends React.Component<{}, State> {
    constructor(props: {}) {
        super(props);
        this.state = {
            settings: null,
            loading: true,
            saving: false,
        };
    }

    componentDidMount = async () => {
        await this.loadSettings();
    }

    loadSettings = async () => {
        this.setState({ loading: true });
        const token = localStorage.getItem("token");
        try {
            const response = await fetch(`${BASE_URL}/api/v1/settings`, {
                headers: {
                    ...(token ? { 'Authorization': `Bearer ${token}` } : {}),
                },
            });
            const json = await response.json();
            if (response.ok && json.data) {
                this.setState({ settings: json.data });
            }
        } catch (error) {
            console.error('Error loading settings:', error);
        } finally {
            this.setState({ loading: false });
        }
    }

    handleSave = async (values: { log_retention_days: number }) => {
        this.setState({ saving: true });
        const token = localStorage.getItem("token");
        try {
            const response = await fetch(`${BASE_URL}/api/v1/settings`, {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json',
                    ...(token ? { 'Authorization': `Bearer ${token}` } : {}),
                },
                body: JSON.stringify(values),
            });
            const json = await response.json();
            if (response.ok) {
                message.success('Settings saved');
                this.setState({ settings: json.data });
            } else {
                message.error(json.message || 'Failed to save settings');
            }
        } catch (error) {
            message.error('Error saving settings');
        } finally {
            this.setState({ saving: false });
        }
    }

    render() {
        if (this.state.loading) {
            return <Flex justify="center" align="center" style={{ minHeight: 200 }}><Title level={4}>Loading...</Title></Flex>;
        }

        return (
            <Flex vertical gap="middle" style={{ maxWidth: 600, margin: '0 auto' }}>
                <Title level={3}>Settings</Title>
                <Card title="Data Retention">
                    <Form
                        layout="vertical"
                        onFinish={this.handleSave}
                        initialValues={this.state.settings || { log_retention_days: 30 }}
                    >
                        <Form.Item
                            label="Log retention (days)"
                            name="log_retention_days"
                            rules={[
                                { required: true, message: 'Please set retention days' },
                                { type: 'number', min: 1, max: 365, message: 'Must be between 1 and 365' },
                            ]}
                        >
                            <InputNumber min={1} max={365} style={{ width: '100%' }} />
                        </Form.Item>
                        <Form.Item>
                            <Button type="primary" htmlType="submit" loading={this.state.saving}>
                                Save Settings
                            </Button>
                        </Form.Item>
                    </Form>
                </Card>
            </Flex>
        );
    }
}