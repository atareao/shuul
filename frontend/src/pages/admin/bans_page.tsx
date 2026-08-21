import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Button, Space, Typography } from 'antd';
import { DeleteFilled, PlusOutlined } from '@ant-design/icons';
import type Ban from "@/models/ban";
import CustomTable from '@/components/custom_table';
import type { FieldDefinition } from '@/common/types';

const { Text } = Typography;
const TITLE = "Active Bans";
const ENDPOINT = "bans";

const FIELDS: FieldDefinition<Ban>[] = [
    { key: 'id', label: 'Id', type: 'number', value: 0, editable: false, fixed: 'left', width: 80 },
    { key: 'ip_address', label: 'IP Address', type: 'string', value: "", width: 150, visible: true },
    { key: 'reason', label: 'Reason', type: 'string', value: "", width: 200, visible: true },
    { key: 'ban_duration_seconds', label: 'Duration (s)', type: 'number', value: 0, width: 120, visible: true },
    { key: 'escalation_level', label: 'Level', type: 'number', value: 0, width: 80, visible: true },
];

export class InnerPage extends React.Component<{ navigate: any; t: any }, {}> {
    private renderHeaderAction = (onCreate: () => void) => {
        return (
            <Button type="primary" onClick={onCreate} icon={<PlusOutlined />}>
                {this.props.t("Ban IP")}
            </Button>
        );
    };

    private renderActionColumn = (item: Ban, _onEdit: any, onDelete: (item: Ban) => void) => {
        return (
            <Space size="middle">
                <Button onClick={() => onDelete(item)} title={this.props.t('Unban')} danger>
                    <DeleteFilled />
                </Button>
            </Space>
        );
    };

    render = () => {
        return (
            <CustomTable<Ban>
                title={TITLE}
                endpoint={ENDPOINT}
                fields={FIELDS}
                t={this.props.t}
                hasActions={true}
                renderHeaderAction={this.renderHeaderAction}
                renderActionColumn={this.renderActionColumn}
            />
        );
    }
}

export default function Page() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}